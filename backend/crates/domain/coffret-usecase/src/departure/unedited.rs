use crate::device_state::LocalEntry;
use crate::local_io_error::LocalIoError;
use crate::local_scan::SourceFile;
use crate::mapped_roots::MappedRoots;

/// Whether a departed file still holds what this device last made it match
/// (spec: EP-15).
///
/// The cheap comparison first, as everywhere a scan compares: a file whose
/// length and modification time are what this device recorded is not opened.
/// One whose observation moved is hashed and held against the hash recorded
/// beside the row, since the Entry's own left the catalog with the Entry. A row
/// that recorded no hash — one an older build wrote — leaves nothing to hold
/// the content against, and the file is kept rather than guessed about.
pub(crate) async fn unedited(
    roots: &dyn MappedRoots,
    source: &SourceFile,
    local: &LocalEntry,
) -> Result<bool, LocalIoError> {
    if local.observation.size == source.size && local.observation.mtime == source.mtime {
        return Ok(true);
    }
    let Some(recorded) = local.observation.hash else {
        return Ok(false);
    };
    Ok(source.content_hash(roots).await? == recorded)
}
