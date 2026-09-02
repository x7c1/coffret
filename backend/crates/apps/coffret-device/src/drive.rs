//! The three things every Drive flow on this device is built from.
//!
//! Creating a Library, renewing its grant, and opening it all need the same
//! transport, the same client credentials, and the same sealed cache, and all
//! three would otherwise assemble them slightly differently. They are here so
//! that the cache one command writes is the cache the next one reads.

use std::sync::Arc;

use coffret_format::{Purpose, PurposeKey};
use coffret_model::MasterKey;
use google_drive_store::{
    AccessTokens, Authorization, ClientCredentials, HttpTransport, OAuthTokens, ReqwestTransport,
    TokenCache,
};

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

/// The Library's grant, sealed under the token-cache purpose key (spec: KD-10).
///
/// The key is derived here rather than in the gateway, and the Master Key it
/// comes from is borrowed rather than handed over: what an adapter keeping a
/// cache for the life of a run needs is the one key that opens that cache, and
/// giving it the Library's Master Key instead would put a second copy of the
/// Library's root secret in a long-lived value (spec: KD-4, DK-7).
pub(crate) fn token_cache(dir: &LibraryDir, master_key: &MasterKey) -> TokenCache {
    let key = PurposeKey::derive(master_key, Purpose::TokenCache);
    TokenCache::new(dir.token_cache_file(), Arc::new(key))
}

/// Asks for a grant on a Library being put on this device, and hands back what
/// a call to Drive is then made through.
///
/// Both flows that put a Library here need a grant before they can say a word to
/// Drive — one to create the app folder, the other to read the name of one — and
/// they need the transport and the tokens together, because the tokens refresh
/// over the same transport the call goes out on.
pub(crate) async fn grant<F>(
    dir: &LibraryDir,
    client_id: &str,
    client_secret: Option<&str>,
    master_key: &MasterKey,
    open_url: F,
) -> Result<(Arc<dyn HttpTransport>, Arc<dyn AccessTokens>)>
where
    F: FnOnce(&str) + Send,
{
    let transport = transport()?;
    let credentials = credentials(client_id, client_secret);
    let cache = token_cache(dir, master_key);

    Authorization::new(Arc::clone(&transport), credentials.clone(), cache.clone())
        .run(open_url)
        .await?;

    let tokens: Arc<dyn AccessTokens> =
        Arc::new(OAuthTokens::new(Arc::clone(&transport), credentials, cache));
    Ok((transport, tokens))
}
