//! The Library's name as it is opened, and the directory an earlier build kept
//! it in.
//!
//! A Library's name is its own and kept in its catalog (spec: FM-15), and every
//! unlock copies it into the settings beside the Library so that this device
//! can list and find the Library by name before the next unlock. A Library an
//! earlier build put here has no name in its catalog: that build named the
//! directory after what the device called the Library and wrote nothing of it
//! to Storage. Opening one gives the Library that name — a record carrying it,
//! with nothing else in it — and moves the directory to the Library's ID. No
//! Pack, Container or Keyring object is rewritten.
//!
//! A server moves the directory before it unlocks anything, since it holds the
//! Library from then on and no move can be made under it; the directory's name
//! is kept in the settings beside the Library until the unlock gives it.
//!
//! Two devices doing this to one Library at once may give it two names; the
//! later record wins, as it does for any name (spec: FM-15).

use std::fs;
use std::sync::Arc;

use coffret_local_fs::UnixFs;
use coffret_model::{LibraryName, Redacted};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::catch_up::{catch_up_catalog, CatchUpRequest};
use coffret_usecase::commit::{commit_batch, CommitRequest, PreparedBatch};
use coffret_usecase::{Index, LibraryKeys, ObjectStore};
use tracing::{info, warn};

use crate::device_settings::DeviceSettings;
use crate::device_time::now;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::library_ref::library_name;
use crate::owner_only;
use crate::server_lock::ServerLock;

/// What this is recorded as.
const OPERATION: &str = "open_library";

/// The Library's name, from its catalog, giving a Library whose catalog names
/// none the name of the directory an earlier build kept it in.
///
/// A catalog naming nothing may only not have caught up — a device that has
/// just joined, say — so it is caught up before anything is committed: a name
/// another device already gave the Library is the Library's, and one committed
/// over it would give the Library another. And a catalog an earlier build
/// kept may have replayed a record naming the Library without keeping the
/// name, since that build knew of no name; so before a Library in a directory
/// an earlier build named is given a name, its records are replayed afresh
/// into a catalog of their own, and a name they give is the one committed —
/// which puts it in this catalog — rather than the directory's.
///
/// The earlier name is the directory's own, or — where the directory was
/// moved to its ID before the Library was given a name — the one the settings
/// kept when it was moved ([`DeviceSettings::earlier_name`]).
///
/// `None` where the catalog names nothing and there is no earlier name either,
/// which is a Library joined before it was ever named: there is nothing to
/// give it.
pub(super) async fn try_read_or_give(
    store: &Arc<dyn ObjectStore>,
    index: &SqliteIndex,
    keys: &LibraryKeys,
    dir: &LibraryDir,
    settings: &DeviceSettings,
) -> Result<Option<LibraryName>> {
    if let Some(name) = index.library_name().await? {
        return Ok(Some(name));
    }
    catch_up(store, index, keys).await?;
    if let Some(name) = index.library_name().await? {
        return Ok(Some(name));
    }
    let Some(earlier) = dir
        .earlier_name(settings.library_id)
        .or(settings.earlier_name.as_deref())
    else {
        return Ok(None);
    };
    let (name, given) = match replayed_name(store, keys, dir).await? {
        Some(name) => (
            name,
            "the name its records gave it, which its catalog had not kept",
        ),
        None => (
            library_name(earlier)?,
            "the name of the directory an earlier build kept it in",
        ),
    };
    commit_batch(CommitRequest::new(
        store.as_ref(),
        index,
        keys.control(),
        PreparedBatch::default().naming(name.clone()),
    ))
    .await
    .map_err(|failure| Error::from(*failure.error))?;
    // The Library's ID and nothing of the name, which is the person's
    // (spec: EL-1).
    info!(
        operation = OPERATION,
        library = %settings.library_id,
        given,
        "named a Library made before Libraries carried a name"
    );
    Ok(Some(name))
}

/// The name the Library's records give it, read by replaying them into a
/// catalog of their own, made for this and removed once it has answered.
async fn replayed_name(
    store: &Arc<dyn ObjectStore>,
    keys: &LibraryKeys,
    dir: &LibraryDir,
) -> Result<Option<LibraryName>> {
    let naming = dir.naming_dir();
    remove(&naming)?;
    owner_only::create_dir(&naming)?;
    let read = async {
        let replayed = SqliteIndex::open(naming.join("index.sqlite"))?;
        catch_up(store, &replayed, keys).await?;
        Ok(replayed.library_name().await?)
    }
    .await;
    if let Err(refused) = remove(&naming) {
        // Nothing depends on it, and the next open that needs it removes it
        // first; the name read is what this was for.
        warn!(
            operation = OPERATION,
            reason = %refused.redacted(),
            "a catalog made to read a Library's name from could not be removed"
        );
    }
    read
}

/// Removes a directory this device made for its own purposes, where absence is
/// the outcome being sought (spec: OC-8).
fn remove(path: &std::path::Path) -> std::result::Result<(), coffret_usecase::LocalIoError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(gone) if gone.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(coffret_usecase::LocalIoError::new(
            coffret_usecase::LocalOperation::Removing,
            path,
            cause,
        )),
    }
}

/// Brings `index` to the Library's head (spec: CK-9).
async fn catch_up(
    store: &Arc<dyn ObjectStore>,
    index: &dyn Index,
    keys: &LibraryKeys,
) -> Result<()> {
    catch_up_catalog(CatchUpRequest::new(
        store.as_ref(),
        index,
        keys,
        &UnixFs::new(),
        now(),
    ))
    .await?;
    Ok(())
}

/// [`try_read_or_give`], where a failure is recorded rather than raised.
///
/// Reading or giving the name reaches Storage, and a device that cannot reach
/// it can still open the Library and read its catalog; the name is read or
/// given the next time it is opened.
pub(super) async fn read_or_give(
    store: &Arc<dyn ObjectStore>,
    index: &SqliteIndex,
    keys: &LibraryKeys,
    dir: &LibraryDir,
    settings: &DeviceSettings,
) -> Option<LibraryName> {
    match try_read_or_give(store, index, keys, dir, settings).await {
        Ok(name) => name,
        Err(refused) => {
            warn!(
                operation = OPERATION,
                library = %settings.library_id,
                reason = %refused.redacted(),
                "the Library's name could not be read or given; it is tried again the next \
                 time the Library is opened"
            );
            None
        }
    }
}

/// Moves the Library in `dir` to the directory its ID names, where an earlier
/// build kept it under another name, and hands back where it now is.
///
/// What a server does before it holds the Library, and so before any unlock:
/// it needs only the ID the settings carry. A server holds the Library for as
/// long as it serves it, so a Library only ever opened by one — through the
/// app — would otherwise never be moved. Where the Library has no name yet,
/// the directory's name is kept in the settings as
/// [`earlier_name`](DeviceSettings::earlier_name) before the directory goes,
/// so the unlock that follows still gives the Library that name.
///
/// # Errors
///
/// Only where the settings cannot be read. A move that cannot be made is
/// recorded and leaves the Library where it is, to be moved the next time it
/// is opened.
pub fn move_to_its_id(dir: LibraryDir) -> Result<LibraryDir> {
    let settings = DeviceSettings::read(&dir)?;
    move_with(dir, &settings)
}

/// Moves the Library in `dir`, whose settings are `settings`, to the directory
/// its ID names, where an earlier build kept it under another name, and hands
/// back where it now is.
///
/// Only while no server holds it (spec: LA-8): a server holds the files in the
/// directory open by their paths, so moving the directory from under one would
/// pull them away from it. Where one does, the Library is left where it is and
/// moved the next time it is opened with none holding it. A directory found at
/// either place is the same Library, so nothing else waits for the move.
///
/// The hold is taken here as a server takes it, and kept across the rename, so
/// a server starting meanwhile is refused rather than serving a directory that
/// is about to move. A move that cannot be made — the ID's directory is taken,
/// the earlier name cannot be kept, or the rename is refused — leaves the
/// Library where it is and is recorded; the Library opens all the same.
pub(super) fn move_with(dir: LibraryDir, settings: &DeviceSettings) -> Result<LibraryDir> {
    let library_id = settings.library_id;
    let Some(earlier) = dir.earlier_name(library_id).map(str::to_owned) else {
        return Ok(dir);
    };
    let target = LibraryDir::of(library_id)?.called(dir.name());
    if target.path().exists() {
        warn!(
            operation = OPERATION,
            library = %library_id,
            "a Library's directory was left where an earlier build kept it: the directory its \
             ID names is taken"
        );
        return Ok(dir);
    }
    let hold = match ServerLock::take(&dir) {
        Ok(hold) => hold,
        Err(Error::LibraryAlreadyServed { .. }) => {
            info!(
                operation = OPERATION,
                library = %library_id,
                "left a Library's directory where an earlier build kept it while a server holds \
                 it; it is moved when it is next opened"
            );
            return Ok(dir);
        }
        Err(refused) => {
            warn!(
                operation = OPERATION,
                library = %library_id,
                reason = %refused.redacted(),
                "a Library's directory was left where an earlier build kept it"
            );
            return Ok(dir);
        }
    };
    // The directory's name is the one a Library nothing has named yet is
    // given at its next unlock, so it is kept before the directory goes.
    if settings.name.is_none() && settings.earlier_name.as_deref() != Some(earlier.as_str()) {
        let mut kept = settings.clone();
        kept.earlier_name = Some(earlier);
        if let Err(refused) = kept.write(&dir) {
            warn!(
                operation = OPERATION,
                library = %library_id,
                reason = %refused.redacted(),
                "a Library's directory was left where an earlier build kept it: the name it \
                 carries could not be kept"
            );
            return Ok(dir);
        }
    }
    let moved = fs::rename(dir.path(), target.path());
    drop(hold);
    match moved {
        Ok(()) => {
            info!(
                operation = OPERATION,
                library = %library_id,
                "moved a Library's directory to the one its ID names"
            );
            Ok(target)
        }
        Err(cause) => {
            let refused =
                Error::local(coffret_usecase::LocalOperation::Renaming, dir.path())(cause);
            warn!(
                operation = OPERATION,
                library = %library_id,
                reason = %refused.redacted(),
                "a Library's directory was left where an earlier build kept it"
            );
            Ok(dir)
        }
    }
}
