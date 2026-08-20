use crate::error::{Error, Result};
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

        let request = HttpRequest {
            method: Method::Post,
            url: self.url.clone(),
            headers: vec![(
                "content-type".to_owned(),
                "application/x-www-form-urlencoded".to_owned(),
            )],
            body: RequestBody::Bytes(body.into_bytes()),
        };

        let response = transport.execute(request).await?;
        let status = response.status();
        let bytes =
            response
                .into_body()
                .into_bytes()
                .await
                .map_err(|error| Error::TokenEndpoint {
                    status,
                    detail: error.to_string(),
                })?;

        if !(200..300).contains(&status) {
            return Err(Error::TokenEndpoint {
                status,
                // The endpoint's own JSON error, which names whether the grant
                // was revoked, expired, or never valid.
                detail: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }

        serde_json::from_slice(&bytes).map_err(|error| Error::TokenEndpoint {
            status,
            detail: format!("unreadable token response: {error}"),
        })
    }
}
