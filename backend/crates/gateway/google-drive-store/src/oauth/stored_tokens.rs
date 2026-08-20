use serde::{Deserialize, Serialize};

/// What is kept between runs so that authorizing is a one-time act.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTokens {
    /// The long-lived token every access token is minted from.
    pub refresh_token: String,
}
