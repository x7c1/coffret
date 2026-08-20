use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;

use crate::error::{Error, Result};
use crate::http::HttpTransport;
use crate::oauth::client_credentials::ClientCredentials;
use crate::oauth::pkce::{random_token, PkceChallenge, CHALLENGE_METHOD};
use crate::oauth::stored_tokens::StoredTokens;
use crate::oauth::token_cache::TokenCache;
use crate::oauth::token_endpoint::{TokenEndpoint, DRIVE_FILE_SCOPE};

mod loopback_redirect;
use loopback_redirect::wait_for_code;

/// Where Google asks the person whether to grant the request.
pub const GOOGLE_AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";

/// How long to wait for the person to finish in their browser.
const REDIRECT_TIMEOUT: Duration = Duration::from_secs(300);

/// The one-time flow that turns a person's consent into a cached grant.
///
/// It is the authorization code flow with PKCE and a loopback redirect, which
/// is what a desktop application is supposed to use: no client secret is
/// trusted, no redirect leaves the machine, and the port is whatever the
/// operating system hands out rather than one fixed number another program
/// could be squatting on.
///
/// Running it needs a person at a browser, so it is deliberately separate from
/// [`OAuthTokens`](crate::OAuthTokens), which runs unattended from what this
/// leaves in the [`TokenCache`].
pub struct Authorization {
    transport: Arc<dyn HttpTransport>,
    credentials: ClientCredentials,
    cache: TokenCache,
    token_endpoint: TokenEndpoint,
    authorization_endpoint: String,
}

impl Authorization {
    /// Takes everything the flow needs but the person.
    pub fn new(
        transport: Arc<dyn HttpTransport>,
        credentials: ClientCredentials,
        cache: TokenCache,
    ) -> Self {
        Self {
            transport,
            credentials,
            cache,
            token_endpoint: TokenEndpoint::default(),
            authorization_endpoint: GOOGLE_AUTHORIZATION_ENDPOINT.to_owned(),
        }
    }

    /// Runs the flow, handing `open` the URL for the person to visit.
    ///
    /// Returns once the grant is cached, so the next run of the application
    /// authorizes itself from the cache and never asks again.
    pub async fn run<F>(&self, open: F) -> Result<()>
    where
        F: FnOnce(&str) + Send,
    {
        let listener =
            TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|error| Error::Authorization {
                    detail: format!("could not listen for the redirect: {error}"),
                })?;

        let port = listener
            .local_addr()
            .map_err(|error| Error::Authorization {
                detail: format!("could not read the redirect port: {error}"),
            })?
            .port();

        let redirect_uri = format!("http://127.0.0.1:{port}");
        let pkce = PkceChallenge::generate()?;
        let state = random_token()?;

        open(&self.authorization_url(&redirect_uri, &pkce, &state));

        let code = tokio::time::timeout(REDIRECT_TIMEOUT, wait_for_code(&listener, &state))
            .await
            .map_err(|_| Error::Authorization {
                detail: format!("no redirect arrived within {}s", REDIRECT_TIMEOUT.as_secs()),
            })??;

        self.exchange(&code, &redirect_uri, &pkce).await
    }

    /// The URL the person visits to grant the request.
    fn authorization_url(&self, redirect_uri: &str, pkce: &PkceChallenge, state: &str) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs([
                ("client_id", self.credentials.client_id()),
                ("redirect_uri", redirect_uri),
                ("response_type", "code"),
                ("scope", DRIVE_FILE_SCOPE),
                ("code_challenge", pkce.challenge()),
                ("code_challenge_method", CHALLENGE_METHOD),
                ("state", state),
                // A refresh token, and one every time: without both of these
                // Google may answer a repeat authorization with an access token
                // alone, and the cache would have nothing durable to keep.
                ("access_type", "offline"),
                ("prompt", "consent"),
            ])
            .finish();

        format!("{}?{query}", self.authorization_endpoint)
    }

    /// Trades the authorization code for a grant and caches it.
    async fn exchange(&self, code: &str, redirect_uri: &str, pkce: &PkceChallenge) -> Result<()> {
        let mut form = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", self.credentials.client_id()),
            ("code_verifier", pkce.verifier()),
        ];
        if let Some(secret) = self.credentials.client_secret() {
            form.push(("client_secret", secret));
        }

        let response = self
            .token_endpoint
            .post(self.transport.as_ref(), &form)
            .await?;

        // What was asked for is not always what was granted, and a grant that
        // reaches more of the account than coffret needs is one to refuse
        // rather than to cache.
        if let Some(scope) = &response.scope {
            if !scope.split(' ').any(|granted| granted == DRIVE_FILE_SCOPE) {
                return Err(Error::Authorization {
                    detail: format!("the grant does not carry {DRIVE_FILE_SCOPE}: {scope}"),
                });
            }
        }

        let refresh_token = response.refresh_token.ok_or(Error::Authorization {
            detail: "the grant carries no refresh token".to_owned(),
        })?;

        self.cache.store(&StoredTokens { refresh_token })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_authorization_url_asks_for_drive_file_and_nothing_else() {
        let authorization = Authorization::new(
            Arc::new(crate::http::ReqwestTransport::with_default_client().unwrap()),
            ClientCredentials::new("client-id"),
            TokenCache::new("/nonexistent/tokens.json"),
        );
        let pkce = PkceChallenge::generate().unwrap();
        let url = authorization.authorization_url("http://127.0.0.1:1234", &pkce, "s3cr3t");
        let parsed = url::Url::parse(&url).expect("the authorization URL must be a URL");

        let scopes: Vec<_> = parsed
            .query_pairs()
            .filter(|(key, _)| key == "scope")
            .map(|(_, value)| value.into_owned())
            .collect();
        assert_eq!(scopes, [DRIVE_FILE_SCOPE]);

        assert!(url.contains("code_challenge_method=S256"));
        assert!(
            !url.contains(pkce.verifier()),
            "the verifier must never leave the process"
        );
    }
}
