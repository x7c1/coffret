use std::io;
use std::path::{Path, PathBuf};

use coffret_format::DecodedEntry;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::debug;

use crate::device_state::{DeviceTime, LocalObservation};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::target::Target;
use crate::index::Index;
use crate::local_mtime::system_time_of;
use crate::local_operation::LocalOperation;
use crate::scratch;

/// Writes one verified Entry into the mapped folder and records having done so.
///
/// The order is what EP-11 asks for and the only order that gives it. The bytes
/// go into a temporary file beside their destination, the file is flushed, it is
/// stamped with the Entry's own modification time, and only then is it renamed
/// onto the final path. A rename within one directory is atomic, so what a
/// reader can see at the target path is either nothing or the whole verified
/// file — never a prefix of one, and never one whose stamp has yet to be set.
///
/// The bookkeeping comes last, and it is not optional: fetching a file is
/// exactly the second way a device materializes an Entry, so from here on the
/// device may report the file as deleted if it goes missing, and the sync flow
/// may offer a change to it back to the Library (spec: EP-10). What is recorded
/// is the Entry's own size and modification time — which is what is now on disk
/// — so the next scan and the next fetch both answer from the cheap comparison
/// and open nothing.
///
/// A failure anywhere along the way takes the temporary file with it. What must
/// not be left behind is a half-written file inside a folder the sync walks: the
/// scratch prefix already keeps a scan from committing one (see
/// [`scratch`](crate::scratch)), and removing it keeps the folder from
/// accumulating them.
pub(super) async fn place(
    index: &dyn Index,
    now: DeviceTime,
    target: &Target,
    entry: &DecodedEntry,
) -> FetchResult<()> {
    let container_id = target.location.container_id;
    let directory = parent(&target.local_path)?;
    fs::create_dir_all(directory)
        .await
        .map_err(|cause| FetchError::Io {
            operation: LocalOperation::Creating,
            path: directory.to_path_buf(),
            cause,
        })?;

    let scratch_path = directory.join(scratch::name(container_id));
    match write_and_rename(&scratch_path, &target.local_path, entry).await {
        Ok(()) => {}
        Err(error) => {
            discard(&scratch_path).await?;
            return Err(error);
        }
    }

    index
        .mark_present(LocalObservation {
            path: target.location.entry.path.clone(),
            size: entry.metadata.size,
            mtime: entry.metadata.mtime,
            at: now,
        })
        .await?;

    debug!(
        container = %container_id,
        bytes = entry.content.len(),
        "placed a fetched Entry and marked it present",
    );
    Ok(())
}

/// The directory the Entry's file belongs in.
///
/// A translated local path always has a parent — it is a mapping's local root
/// with at least one component pushed onto it — so the absence of one is a
/// broken translation rather than a filesystem answer.
fn parent(local_path: &Path) -> FetchResult<&Path> {
    local_path.parent().ok_or_else(|| FetchError::Io {
        operation: LocalOperation::Creating,
        path: local_path.to_path_buf(),
        cause: io::Error::new(
            io::ErrorKind::InvalidInput,
            "a local path with no directory above it",
        ),
    })
}

/// Puts the bytes on disk, stamps them, and moves them onto the final path.
async fn write_and_rename(
    scratch_path: &Path,
    local_path: &Path,
    entry: &DecodedEntry,
) -> FetchResult<()> {
    write(scratch_path, &entry.content).await?;
    stamp(scratch_path, entry).await?;
    fs::rename(scratch_path, local_path)
        .await
        .map_err(|cause| FetchError::Io {
            operation: LocalOperation::Renaming,
            path: local_path.to_path_buf(),
            cause,
        })
}

/// Writes the plaintext out and flushes it to the device.
///
/// Flushed before the rename because the rename is what publishes the file: a
/// crash that reordered the two would leave a name promising content the device
/// never wrote.
async fn write(scratch_path: &Path, content: &[u8]) -> FetchResult<()> {
    fn refused(
        scratch_path: &Path,
        operation: LocalOperation,
    ) -> impl FnOnce(io::Error) -> FetchError + '_ {
        move |cause| FetchError::Io {
            operation,
            path: scratch_path.to_path_buf(),
            cause,
        }
    }

    let mut file = fs::File::create(scratch_path)
        .await
        .map_err(refused(scratch_path, LocalOperation::Creating))?;
    file.write_all(content)
        .await
        .map_err(refused(scratch_path, LocalOperation::Writing))?;
    file.sync_all()
        .await
        .map_err(refused(scratch_path, LocalOperation::Flushing))?;
    Ok(())
}

/// Sets the file's modification time to the one the Entry carries (spec: FM-9).
///
/// Set on the temporary file rather than after the rename, so the file that
/// appears at the final path is already stamped: a scan that ran between the two
/// would otherwise see a file whose time is neither the Entry's nor anything the
/// device wrote down.
///
/// Synchronous, because setting times is a metadata call on a handle and
/// `tokio::fs` offers none; it is one syscall on a file this run just wrote.
async fn stamp(scratch_path: &Path, entry: &DecodedEntry) -> FetchResult<()> {
    let refused = |cause| FetchError::Io {
        operation: LocalOperation::Stamping,
        path: scratch_path.to_path_buf(),
        cause,
    };

    let modified = system_time_of(entry.metadata.mtime).ok_or_else(|| {
        refused(io::Error::new(
            io::ErrorKind::InvalidInput,
            "an Entry's modification time this platform's clock cannot reach",
        ))
    })?;
    let path: PathBuf = scratch_path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::File::options()
            .write(true)
            .open(&path)?
            .set_times(std::fs::FileTimes::new().set_modified(modified))
    })
    .await
    .map_err(|joined| refused(io::Error::other(joined)))?
    .map_err(refused)
}

/// Removes a temporary file a failed placement left.
///
/// One that is already gone is the same outcome as one this call removed, so a
/// cleanup that races the failure it is cleaning up after still succeeds.
async fn discard(scratch_path: &Path) -> FetchResult<()> {
    match fs::remove_file(scratch_path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(FetchError::Io {
            operation: LocalOperation::Removing,
            path: scratch_path.to_path_buf(),
            cause,
        }),
    }
}
