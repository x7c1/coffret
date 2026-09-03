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
    /// The scopes actually granted, as the space-delimited list RFC 6749 §3.3
    /// defines. It is how an over-broad grant shows up, and it is read as a
    /// [`GrantedScopes`](crate::oauth::GrantedScopes) set rather than as text.
    pub scope: Option<String>,
}
