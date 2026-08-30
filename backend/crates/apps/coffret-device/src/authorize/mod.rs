use google_drive_store::Authorization;
use tracing::info;

use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::drive;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Runs the authorization flow again for a Library that already exists.
///
/// A grant is not forever. While a consent screen is in testing Google expires
/// a refresh token after a week, and a person may revoke one at any time, so
/// renewing a grant is an ordinary thing a device does rather than a repair.
/// The Library is otherwise untouched: the Master Key, the catalog, and the
/// mappings are what they were, and only the sealed cache is replaced.
///
/// The Passphrase is asked for once the Library has been found and found to be
/// on Drive, and before anything else: the cache is sealed under the Master Key
/// (spec: KD-10), so there is nothing to write without it, and a Library that is
/// not here or is not on Drive is a refusal that needs no key. A Passphrase that
/// does not open the stored form ends the call with the cache exactly as it was
/// (spec: DK-2, DK-5), and so does a flow the person abandons: the gateway
/// writes the new cache only once the grant is in hand, and writes it through a
/// rename.
pub async fn authorize<P, F>(name: &str, enter_passphrase: P, open_url: F) -> Result<()>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
    F: FnOnce(&str) + Send,
{
    let dir = LibraryDir::resolve(name)?;
    let settings = DeviceSettings::read(&dir)?;

    let ProviderSettings::Drive {
        client_id,
        client_secret,
        ..
    } = &settings.provider
    else {
        return Err(Error::NotADriveLibrary {
            name: dir.name().to_owned(),
        });
    };

    let unlocked = StoredMasterKeyFile::unlock(&dir, &enter_passphrase()?)?;
    let transport = drive::transport()?;
    let credentials = drive::credentials(client_id, client_secret.as_deref());
    let cache = drive::token_cache(&dir, unlocked.master_key);

    Authorization::new(transport, credentials, cache)
        .run(open_url)
        .await?;

    // Ordinary progress, and the one event that says a device's access to a
    // Library was renewed rather than granted. The Library ID names the Library
    // and is not key material; no part of the grant is recorded.
    info!(
        operation = "authorize",
        library = %settings.library_id,
        "renewed this device's grant"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
