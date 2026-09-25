//! The things every Drive flow on this device is built from.
//!
//! Creating a Library, joining one, renewing a grant, and opening a Library all
//! need the same transport, the same client credentials, and the same sealed
//! cache, and all four would otherwise assemble them slightly differently. They
//! are here so that the cache one command writes is the cache the next one
//! reads.

use std::sync::Arc;

use coffret_format::{Purpose, PurposeKey};
use coffret_model::{AccountCacheKey, MasterKey};
use google_drive_store::{
    AccessTokens, Authorization, ClientCredentials, HttpTransport, OAuthTokens, ReqwestTransport,
    TokenCache,
};

use crate::account_dir::AccountDir;
use crate::error::Result;
use crate::library_dir::LibraryDir;

/// The transport every call to Drive goes out through.
pub(crate) fn transport() -> Result<Arc<dyn HttpTransport>> {
    let transport = ReqwestTransport::with_default_client()?;
    Ok(Arc::new(transport))
}

/// Which OAuth client this device authorizes as.
pub(crate) fn credentials(client_id: &str, client_secret: Option<&str>) -> ClientCredentials {
    let credentials = ClientCredentials::new(client_id);
    match client_secret {
        Some(secret) => credentials.with_client_secret(secret),
        None => credentials,
    }
}

/// The account's grant, sealed under its account-cache key (spec: KD-10,
/// KD-12).
///
/// The one cache every Library that references the account reaches Drive
/// through (spec: SA-8). The key arrives already unwrapped from a Library's
/// envelope, and is shared rather than copied: what an adapter keeping a cache
/// for the life of a run needs is the one key that opens that cache, and
/// nothing that opens anything else (spec: DK-7).
pub(crate) fn token_cache(account: &AccountDir, key: Arc<AccountCacheKey>) -> TokenCache {
    TokenCache::for_account(account.token_cache_file(), key)
}

/// A Library's previous per-Library grant, sealed under its token-cache purpose
/// key (spec: KD-10).
///
/// Read only to promote it into an account's cache (spec: SA-8). The key is
/// derived here from a borrowed Master Key, so the Library's root secret is not
/// handed to a value that outlives the call (spec: KD-4, DK-7).
pub(crate) fn previous_token_cache(dir: &LibraryDir, master_key: &MasterKey) -> TokenCache {
    let key = PurposeKey::derive(master_key, Purpose::TokenCache);
    TokenCache::new(dir.previous_token_cache_file(), Arc::new(key))
}

/// The access tokens a call to Drive is made with, minted from `cache`.
pub(crate) fn tokens(
    transport: &Arc<dyn HttpTransport>,
    credentials: ClientCredentials,
    cache: TokenCache,
) -> Arc<dyn AccessTokens> {
    Arc::new(OAuthTokens::new(Arc::clone(transport), credentials, cache))
}

/// Asks the person for a grant and keeps it in `cache`, replacing whatever was
/// cached there.
///
/// The one moment anything is written to an account's cache, and the one that
/// verifies the grant first (spec: SA-4, SA-6). A flow the person abandons
/// leaves the cache exactly as it was: the gateway writes only once the grant
/// is in hand, and writes through a rename.
pub(crate) async fn consent<F>(
    transport: &Arc<dyn HttpTransport>,
    credentials: ClientCredentials,
    cache: TokenCache,
    open_url: F,
) -> Result<()>
where
    F: FnOnce(&str) + Send,
{
    Authorization::new(Arc::clone(transport), credentials, cache)
        .run(open_url)
        .await?;
    Ok(())
}
