use async_trait::async_trait;

use crate::error::Result;

/// Where the gateway gets a credential to sign a call with.
///
/// It is a trait, and not the concrete OAuth implementation, for the same
/// reason the transport is: the behaviour worth testing is what the gateway
/// does around a token — that a 401 costs exactly one refresh and then stops —
/// and testing it must not involve a real grant.
#[async_trait]
pub trait AccessTokens: Send + Sync {
    /// A token to authorize the next call with, minting one if needed.
    async fn access_token(&self) -> Result<String>;

    /// Discards the current token and mints a new one.
    ///
    /// Called when Drive rejects a token the gateway believed was still good,
    /// which happens whenever a grant is changed out from under it.
    async fn refresh(&self) -> Result<String>;
}
