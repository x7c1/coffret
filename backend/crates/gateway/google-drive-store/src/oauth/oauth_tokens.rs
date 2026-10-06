use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::error::{Error, Result};
use crate::http::HttpTransport;
use crate::oauth::access_tokens::AccessTokens;
use crate::oauth::client_credentials::ClientCredentials;
use crate::oauth::token_cache::TokenCache;
use crate::oauth::token_endpoint::TokenEndpoint;

/// A token and the moment it stops being usable.
struct ActiveToken {
    value: String,
    good_until: Instant,
}

/// How far before its stated expiry a token is treated as spent.
///
/// A token that expires between being read here and arriving at Drive would
/// cost a round trip and a refresh; retiring it early costs nothing.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// How long to trust a token the endpoint did not put an expiry on.
const ASSUMED_LIFETIME: Duration = Duration::from_secs(300);

/// Access tokens minted from a cached refresh token (spec: SA-6).
///
/// The refresh token is the durable half of the grant and lives in the
/// [`TokenCache`]; access tokens are short-lived, kept in memory only, and
/// minted again whenever the last one is spent. Authorizing — which needs a
/// person at a browser — happens once, in the authorization flow, and never
/// here.
pub struct OAuthTokens {
    transport: Arc<dyn HttpTransport>,
    credentials: ClientCredentials,
    cache: TokenCache,
    endpoint: TokenEndpoint,
    active: Mutex<Option<ActiveToken>>,
}

impl OAuthTokens {
    /// Takes everything needed to mint access tokens from a cached grant.
    pub fn new(
        transport: Arc<dyn HttpTransport>,
        credentials: ClientCredentials,
        cache: TokenCache,
    ) -> Self {
        Self {
            transport,
            credentials,
            cache,
            endpoint: TokenEndpoint::default(),
            active: Mutex::new(None),
        }
    }

    /// Mints a new access token from the cached refresh token.
    async fn mint(&self) -> Result<ActiveToken> {
        let stored = self.cache.load()?.ok_or(Error::NotAuthorized)?;

        let mut form = vec![
            ("grant_type", "refresh_token"),
            ("refresh_token", stored.refresh_token.as_str()),
            ("client_id", self.credentials.client_id()),
        ];
        if let Some(secret) = self.credentials.client_secret() {
            form.push(("client_secret", secret));
        }

        let response = self.endpoint.post(self.transport.as_ref(), &form).await?;
        let lifetime = response
            .expires_in
            .map(Duration::from_secs)
            .unwrap_or(ASSUMED_LIFETIME);

        Ok(ActiveToken {
            value: response.access_token,
            good_until: Instant::now() + lifetime.saturating_sub(EXPIRY_MARGIN),
        })
    }
}

#[async_trait]
impl AccessTokens for OAuthTokens {
    async fn access_token(&self) -> Result<String> {
        let mut active = self.active.lock().await;
        if let Some(token) = active.as_ref() {
            if Instant::now() < token.good_until {
                return Ok(token.value.clone());
            }
        }

        let token = self.mint().await?;
        let value = token.value.clone();
        *active = Some(token);
        Ok(value)
    }

    async fn refresh(&self) -> Result<String> {
        // The lock is held across the mint so that several calls that all hit a
        // 401 at once refresh the grant once between them rather than once each.
        let mut active = self.active.lock().await;
        let token = self.mint().await?;
        let value = token.value.clone();
        *active = Some(token);
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    //! What a running process mints from once the grant under it has been
    //! renewed (spec: SA-6).

    use std::sync::Arc;

    use coffret_format::{Purpose, PurposeKey};
    use coffret_model::MasterKey;

    use super::OAuthTokens;
    use crate::http::{StubAnswer, StubTransport};
    use crate::oauth::access_tokens::AccessTokens;
    use crate::oauth::client_credentials::ClientCredentials;
    use crate::oauth::stored_tokens::StoredTokens;
    use crate::oauth::token_cache::TokenCache;

    const EXPIRED: &str = "1//0gTheGrantThatRanOut";
    const RENEWED: &str = "1//0gTheGrantJustGiven";

    // A grant that ran out is renewed into the same cache while a process that
    // opened it is still running — the explorer's server, reconnecting. What
    // that process mints next is minted from the renewed grant, because the
    // cache is read at every mint rather than once when the store was built: no
    // reopening, and nothing to swap.
    #[tokio::test]
    async fn the_next_mint_after_a_renewal_uses_the_renewed_grant() {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let key = PurposeKey::derive(
            &MasterKey::from_bytes([0x3d; MasterKey::BYTE_LEN]),
            Purpose::TokenCache,
        );
        let cache = TokenCache::new(directory.path().join("tokens.bin"), Arc::new(key));
        cache
            .store(&StoredTokens {
                refresh_token: EXPIRED.to_owned(),
            })
            .expect("storing must succeed");
        let transport = StubTransport::new([
            StubAnswer::json(
                400,
                r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
            ),
            StubAnswer::json(200, r#"{"access_token":"ya29.Renewed","expires_in":3599}"#),
        ]);
        let tokens = OAuthTokens::new(
            Arc::clone(&transport) as _,
            ClientCredentials::new("client-id"),
            cache.clone(),
        );

        tokens
            .access_token()
            .await
            .expect_err("a grant that ran out mints nothing");

        cache
            .store(&StoredTokens {
                refresh_token: RENEWED.to_owned(),
            })
            .expect("the renewal writes the same cache");
        let token = tokens
            .access_token()
            .await
            .expect("the renewed grant mints a token");

        assert_eq!(token, "ya29.Renewed");
        let body = String::from_utf8(transport.request(1).body).expect("a form is text");
        assert!(
            body.contains("0gTheGrantJustGiven") && !body.contains("0gTheGrantThatRanOut"),
            "the mint after the renewal carries the renewed grant: {body}",
        );
    }
}
