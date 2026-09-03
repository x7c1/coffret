use coffret_logging::redact;
use tracing::warn;

use crate::answer_ceiling::MAX_DOCUMENT_LEN;
use crate::error::{Error, Result, TokenResponseDefect};
use crate::http::{HttpRequest, HttpTransport, Method, RequestBody};
use crate::oauth::token_response::TokenResponse;

/// Where Google mints and refreshes access tokens.
pub const GOOGLE_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// The one Drive permission coffret asks for.
///
/// `drive.file` reaches only the files this application itself created, so
/// authorizing coffret does not hand it the rest of the account's Drive. It is
/// enough for a Library — every Storage Object in one was written by coffret —
/// and asking for more would be asking for access that no part of the design
/// uses.
///
/// It is what is asked for and equally what is accepted: the scopes a token
/// response says were granted have to be this one and no other, or the
/// authorization flow refuses the answer and caches nothing. What is cached is
/// still a bearer credential for every object in the Library; what the check
/// keeps it from being is a credential for the rest of the account.
pub const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

/// Where token requests are posted, and how the answer is read.
///
/// Both the one-time code exchange and every later refresh go through here, so
/// the endpoint's URL and the shape of its answer are stated once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenEndpoint {
    url: String,
}

impl Default for TokenEndpoint {
    fn default() -> Self {
        Self::new(GOOGLE_TOKEN_ENDPOINT)
    }
}

impl TokenEndpoint {
    /// Points at a token endpoint.
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// Posts a form to the endpoint and reads the tokens it answers with.
    pub async fn post(
        &self,
        transport: &dyn HttpTransport,
        form: &[(&str, &str)],
    ) -> Result<TokenResponse> {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form.iter().copied())
            .finish();

        let mut request = HttpRequest::new(Method::Post, self.url.clone())
            .with_header("content-type", "application/x-www-form-urlencoded");
        request.body = RequestBody::Bytes(body.into_bytes());

        let response = transport.execute(request).await?;
        let status = response.status();
        // A token response is a handful of fields and one access token, and a
        // refusal is this endpoint's own JSON error. What bounds the read is
        // what such a document can be: the endpoint is no more inside the trust
        // boundary than Storage is, and nothing here is authenticated before it
        // is held.
        let bytes = response
            .into_body()
            .into_bytes_within(MAX_DOCUMENT_LEN)
            .await
            .map_err(|cause| Error::UnreadableTokenResponse {
                status,
                cause: TokenResponseDefect::Body(cause),
            })?;

        if !(200..300).contains(&status) {
            // The endpoint's own JSON error, which names whether the grant was
            // revoked, expired, or never valid. It reaches the log with any
            // credential taken out of it: what this endpoint refused with is
            // worth keeping, and its answers are the one place a token could
            // turn up in a body.
            let detail = redact::body(&bytes);
            warn!(
                operation = "mint_access_token",
                status,
                body = %detail,
                "the token endpoint refused to mint an access token"
            );
            return Err(Error::TokenEndpoint { status, detail });
        }

        serde_json::from_slice(&bytes).map_err(|cause| {
            warn!(
                operation = "mint_access_token",
                status,
                detail = %cause,
                // The body is left out of this one alone: this answer was a
                // successful mint, so it holds a token, and an answer this
                // build could not read is no reason to write one to a file.
                "the token endpoint answered with something this build cannot read"
            );
            Error::UnreadableTokenResponse {
                status,
                cause: TokenResponseDefect::Document(cause),
            }
        })
    }
}
