use std::sync::Arc;

use coffret_local_fs::UnixFs;
use coffret_model::Passphrase;
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::LibraryKeys;
use tracing::info;

use super::{store, OpenLibrary};
use crate::device_settings::DeviceSettings;
use crate::error::Result;
use crate::library_dir::LibraryDir;
use crate::reach::Reach;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Opens the Library called `name` with the Passphrase that protects its Master
/// Key.
///
/// The order matters: the settings are read, then the Passphrase is asked for
/// and spent, and only then is anything built. A name that is not one path
/// component and a Library that is not on this device are both refused before
/// `enter_passphrase` is called at all, and a Passphrase that does not open the
/// stored form ends the call having read two files and written none
/// (spec: DK-5) — which is what makes a mistyped Passphrase cost nothing but
/// the typing.
///
/// A Drive Library opens the grant of the account it references through its
/// envelope, and a Library a build before accounts put here has its own grant
/// promoted into an account first, the first time it is opened (spec: SA-8).
pub async fn open_library<P>(name: &str, enter_passphrase: P) -> Result<OpenLibrary>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library_through(&Reach::this_device(), name, enter_passphrase).await
}

/// [`open_library`], reaching Drive through `reach`.
pub(crate) async fn open_library_through<P>(
    reach: &Reach,
    name: &str,
    enter_passphrase: P,
) -> Result<OpenLibrary>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    let dir = LibraryDir::resolve(name)?;
    let mut settings = DeviceSettings::read(&dir)?;
    let passphrase = enter_passphrase()?;
    let unlocked = StoredMasterKeyFile::unlock(&dir, &passphrase)?;

    let store = store::build(
        reach,
        &dir,
        &mut settings,
        &unlocked.master_key,
        &passphrase,
    )
    .await?;
    let index = SqliteIndex::open(dir.index_file())?;

    // Ordinary progress: which Library was opened, on which provider, at which
    // epoch. None of the three is key material or a path.
    info!(
        operation = "open_library",
        library = %settings.library_id,
        provider = settings.provider.kind(),
        epoch = unlocked.epoch.get(),
        "opened a Library"
    );
    Ok(OpenLibrary {
        store,
        index: Arc::new(index),
        local_fs: Arc::new(UnixFs::new()),
        keys: LibraryKeys::derive(&unlocked.master_key, unlocked.epoch),
        spool: dir.spool_dir(),
        library_id: settings.library_id,
        epoch: unlocked.epoch,
        provider: settings.provider.kind(),
    })
}
