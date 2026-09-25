//! Where one account's grant is kept on this device.

use std::path::{Path, PathBuf};

use crate::account_name::AccountName;
use crate::error::Result;
use crate::library_dir::{accounts_root, STAGING_SUFFIX};

/// The file an account's grant is kept in, sealed under its account-cache key
/// (spec: KD-10, KD-12).
const TOKEN_CACHE_FILE: &str = "token-cache.cftc";
/// The file that says which OAuth client the account's grant was issued to.
const SETTINGS_FILE: &str = "settings.json";

/// One account's directory under the device's accounts, and the two things in
/// it (spec: SA-8).
///
/// ```text
/// coffret/accounts/<account name>/
///   token-cache.cftc   the account's grant, sealed under its account-cache key
///   settings.json      the OAuth client the grant was issued to
/// ```
///
/// One per account and never a second: every Library that references the
/// account reaches Storage through this one cache, each opening it through the
/// envelope in its own directory (spec: SA-9). Nothing in here says which
/// Libraries those are — each Library's settings name its account, and that is
/// the one answer to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AccountDir {
    name: AccountName,
    path: PathBuf,
}

impl AccountDir {
    /// Works out where the account called `name` is kept.
    pub(crate) fn resolve(name: &AccountName) -> Result<Self> {
        Ok(Self {
            name: name.clone(),
            path: accounts_root()?.join(name.as_str()),
        })
    }

    /// The account's name on this device.
    pub(crate) fn name(&self) -> &AccountName {
        &self.name
    }

    /// The directory the account's grant is kept in.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// The directory an account of this name is built in before any Library
    /// references it.
    ///
    /// No account name holds a `.`, so none can be one of these.
    pub(crate) fn staging(&self) -> Self {
        Self {
            name: self.name.clone(),
            path: self
                .path
                .with_file_name(format!("{}{STAGING_SUFFIX}", self.name)),
        }
    }

    /// The account's sealed grant.
    pub(crate) fn token_cache_file(&self) -> PathBuf {
        self.path.join(TOKEN_CACHE_FILE)
    }

    /// The OAuth client the account's grant was issued to.
    pub(crate) fn settings_file(&self) -> PathBuf {
        self.path.join(SETTINGS_FILE)
    }

    /// Whether an account of this name is on this device.
    ///
    /// The settings are what is asked about, for the reason a Library's are: a
    /// directory without them is not one anything can be renewed through.
    pub(crate) fn is_present(&self) -> bool {
        self.settings_file().is_file()
    }
}
