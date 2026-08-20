use serde::Deserialize;

/// What the token endpoint answers with.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    /// The token to authorize calls with.
    pub access_token: String,
    /// How many seconds the access token is good for.
    pub expires_in: Option<u64>,
    /// The long-lived token, present only when a code is first exchanged.
    pub refresh_token: Option<String>,
    /// The scopes actually granted, which is how an over-broad grant would show
    /// up.
    pub scope: Option<String>,
}
