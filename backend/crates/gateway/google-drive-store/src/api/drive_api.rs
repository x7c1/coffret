use std::sync::Arc;

use coffret_usecase::Result;

use crate::api::endpoints::Endpoints;
use crate::http::{HttpRequest, HttpResponse, HttpTransport};
use crate::oauth::AccessTokens;

/// Makes authorized calls against Drive.
///
/// Everything that is true of every call lives here: the bearer header, and
/// what to do when Drive says the token is no good. A grant can be refreshed
/// out from under a running process at any moment, so one 401 is answered by
/// minting a token and trying once more — and exactly once more. Looping on it
/// would turn a revoked grant into an endless retry against an endpoint that is
/// never going to say yes.
pub struct DriveApi {
    transport: Arc<dyn HttpTransport>,
    tokens: Arc<dyn AccessTokens>,
    endpoints: Endpoints,
}

impl DriveApi {
    /// Takes a transport, a source of tokens, and the URLs to call.
    pub fn new(
        transport: Arc<dyn HttpTransport>,
        tokens: Arc<dyn AccessTokens>,
        endpoints: Endpoints,
    ) -> Self {
        Self {
            transport,
            tokens,
            endpoints,
        }
    }

    /// The URLs this API is reached at.
    pub fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    /// Makes a call, refreshing the token once if Drive rejects it.
    ///
    /// The request is built from the token rather than passed in, because the
    /// retry needs a second one carrying the new token — and because a request
    /// is consumed by being sent.
    pub async fn send<F>(&self, build: F) -> Result<HttpResponse>
    where
        F: Fn(&str) -> HttpRequest + Send + Sync,
    {
        let token = self.tokens.access_token().await?;
        let response = self.transport.execute(build(&token)).await?;
        if response.status() != 401 {
            return Ok(response);
        }

        let token = self.tokens.refresh().await?;
        // Whatever comes back now is the answer, 401 included: a token minted
        // seconds ago being refused means the grant itself is gone.
        Ok(self.transport.execute(build(&token)).await?)
    }

    /// Makes a call that cannot be repeated.
    ///
    /// An upload carries a stream, and a stream that has been sent cannot be
    /// sent again, so there is nothing to retry with: a 401 here fails the
    /// whole upload rather than costing a refresh.
    pub async fn send_once<F>(&self, build: F) -> Result<HttpResponse>
    where
        F: FnOnce(&str) -> HttpRequest + Send,
    {
        let token = self.tokens.access_token().await?;
        Ok(self.transport.execute(build(&token)).await?)
    }
}

/// The header that carries an access token.
pub fn authorization(token: &str) -> (String, String) {
    ("authorization".to_owned(), format!("Bearer {token}"))
}
