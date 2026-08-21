use std::sync::Arc;

use coffret_logging::redact;
use coffret_usecase::Result;
use tracing::{debug, warn};

use crate::api::endpoints::Endpoints;
use crate::http::{HttpRequest, HttpResponse, HttpTransport};
use crate::oauth::AccessTokens;

/// The status that means the token was refused.
const UNAUTHORIZED: u16 = 401;

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
        let response = self.call(build(&token)).await?;
        if response.status() != UNAUTHORIZED {
            return Ok(response);
        }

        let token = self.tokens.refresh().await?;
        // Whatever comes back now is the answer, 401 included: a token minted
        // seconds ago being refused means the grant itself is gone.
        let response = self.call(build(&token)).await?;
        if response.status() == UNAUTHORIZED {
            // The one place this gateway gives up after trying again. How long
            // was spent waiting is not recorded because nothing waited: a
            // refusal of a fresh token is answered at once, and the accounting
            // of backoff belongs to the retry policy that does the waiting.
            warn!(
                attempts = 2,
                "gave up: a token minted seconds ago was refused as well, so the grant is gone"
            );
        }
        Ok(response)
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
        self.call(build(&token)).await
    }

    /// Performs one call and records that it happened.
    ///
    /// What a single call did is ordinary detail, so it is recorded at `debug`:
    /// worth having when a run is being investigated, and too much to keep for
    /// every run. The method, the endpoint, and the status are the whole of it:
    /// no header is ever recorded, one of them being the `Authorization` header
    /// that carries the access token.
    async fn call(&self, request: HttpRequest) -> Result<HttpResponse> {
        let method = request.method;
        // The path of a URL of Drive's is a file id or a folder id: opaque
        // values that say nothing about the Library they belong to. The query
        // is a different matter — the URL an upload is sent to is the session
        // URI Drive minted, and its `upload_id` is a capability — so it is cut
        // off rather than recorded.
        let url = redact::url(&request.url).to_owned();

        let response = self.transport.execute(request).await?;
        debug!(
            ?method,
            url,
            status = response.status(),
            "Storage answered a call"
        );
        Ok(response)
    }
}

/// The header that carries an access token.
pub fn authorization(token: &str) -> (String, String) {
    ("authorization".to_owned(), format!("Bearer {token}"))
}
