//! The account-cache key envelope a Library holds for the account it
//! references.

use std::fs;
use std::io::ErrorKind;

use coffret_format::{
    decode_account_cache_key_envelope, encode_account_cache_key_envelope, Purpose, PurposeKey,
};
use coffret_model::{AccountCacheKey, MasterKey};
use coffret_usecase::{LocalIoError, LocalOperation};

use crate::account_name::AccountName;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::owner_only;

/// The envelope file in one Library's directory: the account's account-cache
/// key, sealed under this Library's `coffret/v1/account-cache-wrap` purpose key
/// and bound to the account name (spec: SA-9, KD-12).
///
/// `coffret-format` says what the bytes are and does no I/O; this says where
/// they go. The purpose key is derived here, borrowed from the Master Key, and
/// dropped when the call ends, so nothing longer-lived than the call holds it
/// (spec: DK-7).
pub(crate) struct AccountEnvelope;

impl AccountEnvelope {
    /// Seals `key` into the Library in `dir`, for the account called `account`.
    pub(crate) fn write(
        dir: &LibraryDir,
        master_key: &MasterKey,
        account: &AccountName,
        key: &AccountCacheKey,
    ) -> Result<()> {
        let wrap = PurposeKey::derive(master_key, Purpose::AccountCacheWrap);
        let envelope = encode_account_cache_key_envelope(key, &wrap, account.as_str())
            .map_err(|cause| Error::KeyMaterial { cause })?;
        owner_only::write_file(&dir.account_envelope_file(), &envelope)
    }

    /// Opens the envelope of the Library in `dir` for the account it names.
    ///
    /// Missing, malformed, or failing to authenticate under this Library's key
    /// and this account's name: each is an unreadable envelope, and never a
    /// Library that references no account — a damaged envelope is not quietly
    /// answered with a second consent (spec: SA-9).
    pub(crate) fn open(
        dir: &LibraryDir,
        master_key: &MasterKey,
        account: &AccountName,
    ) -> Result<AccountCacheKey> {
        let unreadable = |cause| Error::UnreadableAccountEnvelope {
            library: dir.name().to_owned(),
            account: account.as_str().to_owned(),
            cause,
        };
        let path = dir.account_envelope_file();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == ErrorKind::NotFound => return Err(unreadable(None)),
            Err(cause) => {
                return Err(LocalIoError::new(LocalOperation::Reading, path, cause).into())
            }
        };
        let wrap = PurposeKey::derive(master_key, Purpose::AccountCacheWrap);
        decode_account_cache_key_envelope(&bytes, &wrap, account.as_str())
            .map_err(|cause| unreadable(Some(Box::new(cause))))
    }

    /// Whether the Library in `dir` holds an envelope at all.
    pub(crate) fn is_present(dir: &LibraryDir) -> bool {
        dir.account_envelope_file().is_file()
    }
}
