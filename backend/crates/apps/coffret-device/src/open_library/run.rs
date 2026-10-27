use std::sync::Arc;

use coffret_local_fs::UnixFs;
use coffret_model::Passphrase;
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::LibraryKeys;
use tracing::info;

use super::{named, store, OpenLibrary};
use crate::device_settings::DeviceSettings;
use crate::error::Result;
use crate::library_dir::LibraryDir;
use crate::library_ref::LibraryRef;
use crate::reach::Reach;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Opens the Library `library` names with the Passphrase that protects its
/// Master Key.
///
/// The order matters: the settings are read, then the Passphrase is asked for
/// and spent, and only then is anything built. A Library that is not on this
/// device, and a name more than one Library here carries, are both refused
/// before `enter_passphrase` is called at all, and a Passphrase that does not
/// open the stored form ends the call having read two files and written none
/// (spec: DK-5) — which is what makes a mistyped Passphrase cost nothing but
/// the typing.
///
/// A Drive Library opens the grant of the account it references through its
/// envelope, and a Library a build before accounts put here has its own grant
/// promoted into an account first, the first time it is opened (spec: SA-8).
///
/// Every open copies the Library's name, as its catalog holds it, into the
/// settings beside it (spec: FM-15). A Library a build before names put here
/// is given the name of the directory that build kept it in — a record
/// carrying the name and nothing else — and moved to the directory its ID
/// names while no server holds it. One a server moved there before it unlocked
/// it ([`move_to_its_id`](crate::move_to_its_id)) is given the name the
/// settings kept from the directory. Settling the name reaches Storage, and a
/// device that cannot reach it opens the Library all the same and tries again
/// at the next open.
pub async fn open_library<P>(library: &LibraryRef, enter_passphrase: P) -> Result<OpenLibrary>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library_through(&Reach::this_device(), library, enter_passphrase).await
}

/// [`open_library`], reaching Drive through `reach`.
pub(crate) async fn open_library_through<P>(
    reach: &Reach,
    library: &LibraryRef,
    enter_passphrase: P,
) -> Result<OpenLibrary>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    let dir = LibraryDir::find(library)?;
    let mut settings = DeviceSettings::read(&dir)?;
    let passphrase = enter_passphrase()?;
    let unlocked = StoredMasterKeyFile::unlock(&dir, &passphrase)?;

    let (store, grant) = store::build(
        reach,
        &dir,
        &mut settings,
        &unlocked.master_key,
        &passphrase,
    )
    .await?;
    let keys = LibraryKeys::derive(&unlocked.master_key, unlocked.epoch);
    let index = SqliteIndex::open(dir.index_file())?;
    let name = named::read_or_give(&store, &index, &keys, &dir, &settings).await;
    // The copy beside the Library, for listing and finding it before the next
    // unlock. Kept as it was where the name could not be read or given.
    // The earlier name a move kept goes once the catalog names the Library,
    // since from then on the copy is what it is listed and found by.
    if let Some(name) = &name {
        if settings.name.as_deref() != Some(name.as_str()) || settings.earlier_name.is_some() {
            settings.name = Some(name.as_str().to_owned());
            settings.earlier_name = None;
            settings.write(&dir)?;
        }
    }
    // Moved only once it has a name, so that the name the directory carried is
    // never lost with it. The catalog is closed across the move — it is a file
    // in the directory — and opened again where the directory went.
    let (dir, index) = match name {
        Some(_) => {
            drop(index);
            let dir = named::move_with(dir, &settings)?;
            let index = SqliteIndex::open(dir.index_file())?;
            (dir, index)
        }
        None => (dir, index),
    };

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
        keys,
        spool: dir.spool_dir(),
        parcel_dir: dir.parcel_dir(),
        parcel_len: coffret_format::PARCEL_LEN,
        library_id: settings.library_id,
        name: name.or_else(|| {
            settings
                .name
                .as_deref()
                .and_then(|copy| coffret_model::LibraryName::parse(copy).ok())
        }),
        epoch: unlocked.epoch,
        provider: settings.provider.kind(),
        grant,
        births: Arc::default(),
        fetch_memory: Default::default(),
        live: Default::default(),
    })
}
