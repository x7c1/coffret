//! The three things every Drive flow on this device is built from.
//!
//! Creating a Library, renewing its grant, and opening it all need the same
//! transport, the same client credentials, and the same sealed cache, and all
//! three would otherwise assemble them slightly differently. They are here so
//! that the cache one command writes is the cache the next one reads.

use std::sync::Arc;

use coffret_model::MasterKey;
use google_drive_store::{ClientCredentials, HttpTransport, ReqwestTransport, TokenCache};

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

/// The Library's grant, sealed under its Master Key (spec: KD-10).
pub(crate) fn token_cache(dir: &LibraryDir, master_key: MasterKey) -> TokenCache {
    TokenCache::new(dir.token_cache_file(), master_key)
}
