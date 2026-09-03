use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;

use crate::error::{Error, RedirectStep, Result};
use crate::http::HttpTransport;
use crate::oauth::client_credentials::ClientCredentials;
use crate::oauth::granted_scopes::GrantedScopes;
use crate::oauth::pkce::{random_token, PkceChallenge, CHALLENGE_METHOD};
use crate::oauth::stored_tokens::StoredTokens;
use crate::oauth::token_cache::TokenCache;
use crate::oauth::token_endpoint::{TokenEndpoint, DRIVE_FILE_SCOPE};

mod loopback_redirect;
use loopback_redirect::wait_for_code;

#[cfg(test)]
mod tests;

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
/// The grant it asks for is [`DRIVE_FILE_SCOPE`] and nothing else, and the
/// grant it keeps is checked to be exactly that: a token response granting
/// anything besides — or naming no scope at all — is refused as
/// [`Error::GrantNotDriveFileAlone`] and nothing reaches the cache.
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

    /// Points the flow at another token endpoint, so a scripted one can answer.
    #[cfg(test)]
    fn with_token_endpoint(mut self, token_endpoint: TokenEndpoint) -> Self {
        self.token_endpoint = token_endpoint;
        self
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
                .map_err(|cause| Error::LoopbackRedirect {
                    step: RedirectStep::Bind,
                    cause,
                })?;

        let port = listener
            .local_addr()
            .map_err(|cause| Error::LoopbackRedirect {
                step: RedirectStep::Port,
                cause,
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
        match &response.scope {
            Some(scope) => {
                let granted = GrantedScopes::parse(scope);
                if !granted.is_drive_file_alone() {
                    return Err(Error::GrantNotDriveFileAlone {
                        granted: Some(granted),
                    });
                }
            }
            // RFC 6749 §5.1 leaves the field out when the grant is identical to
            // the request, and Google always sends it. Silence is refused all
            // the same: the invariant is that the grant was *verified* to be
            // DRIVE_FILE_SCOPE alone, and an endpoint that says nothing
            // verifies nothing. The cost of holding that line is that a
            // provider which stopped sending the field would fail this flow
            // closed, with a refusal that says exactly why.
            None => return Err(Error::GrantNotDriveFileAlone { granted: None }),
        }

        let refresh_token = response.refresh_token.ok_or(Error::Authorization {
            detail: "the grant carries no refresh token".to_owned(),
        })?;

        self.cache.store(&StoredTokens { refresh_token })
    }
}
