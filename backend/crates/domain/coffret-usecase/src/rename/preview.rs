use crate::commit::catch_up;
use crate::departure::{self, mapped_file, MappedFile};
use crate::device_state::LocalEntryState;
use crate::fetch::reaching;
use crate::index::Index;
use crate::mapped_roots::MappedRoots;
use crate::relocate::follows;
use crate::rename::plan::{self, Plan};
use crate::rename::rename_error::{RenameError, RenameResult};
use crate::rename::rename_preview::RenamePreview;
use crate::rename::rename_request::RenameRequest;
use crate::sync::{complete_after_catch_up, PendingWork};

/// Works out what [`rename_entries`](super::rename_entries) would do with the
/// same request, and writes nothing to the Library or to this device's disk
/// (spec: FM-15, EP-10).
///
/// The run's own first steps: the catalog caught up to the head and a batch of
/// this device's that landed completed (spec: CK-9, OC-7), then the plan, so
/// the two cannot disagree about which Entries are renamed. A refusal of the
/// set is an answer here rather than an error: it is what a person is shown
/// before they are offered the rename.
///
/// Where the request carries this device's disk
/// ([`RenameRequest::with_local_disk`]), each of its files of the renamed
/// Entries is counted as one that would follow its Entry, one that would leave
/// this device, or one edited since it was placed. A file the device placed and
/// that is gone counts as none of them: there is nothing on disk to move. One
/// that cannot be read counts as edited.
///
/// # Errors
///
/// Only what reaching Storage and reading the catalog fail with; never
/// [`RenameError::Refused`].
pub async fn preview_rename(request: RenameRequest<'_>) -> RenameResult<RenamePreview> {
    let RenameRequest {
        store,
        index,
        keys,
        spool,
        pairs,
        policy,
        now,
        disk,
        ..
    } = request;

    let pairs = match plan::pairs_of(&pairs) {
        Ok(pairs) => pairs,
        Err(refusal) => return Ok(refused(refusal)),
    };
    catch_up(store, index, keys.control(), &policy.retry).await?;
    // Unowned: a preview holds none of this device's pending work, and leaves
    // the rows to a run that does (spec: OC-2, OC-7).
    complete_after_catch_up(index, spool, now, PendingWork::Unowned).await;
    let Plan { renamed, .. } = match plan::plan(index, &pairs).await {
        Ok(plan) => plan,
        Err(RenameError::Refused(refusal)) => return Ok(refused(refusal)),
        Err(other) => return Err(other),
    };

    let mut preview = RenamePreview {
        renamed,
        ..RenamePreview::default()
    };
    if let Some(disk) = disk {
        count_local(index, disk.roots, &mut preview).await?;
    }
    Ok(preview)
}

fn refused(refusal: crate::rename::RenameRefusal) -> RenamePreview {
    RenamePreview {
        refused: Some(refusal),
        ..RenamePreview::default()
    }
}

/// Counts this device's files of the Entries `preview` renames into it.
async fn count_local(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    preview: &mut RenamePreview,
) -> RenameResult<()> {
    let mappings = index.mappings().await?;
    for one in &preview.renamed {
        let Some(local) = index.local_entry_at(&one.from).await? else {
            continue;
        };
        if local.state != LocalEntryState::Present || local.awaits_relocation() {
            continue;
        }
        let Some(mapping) = reaching(&mappings, &one.from) else {
            continue;
        };
        let source = match mapped_file(roots, mapping, &one.from).await {
            Ok(MappedFile::File(source)) => source,
            // Nothing there to move: gone, or not a file this device could
            // relocate, which the run reports when it meets it.
            Ok(_) => continue,
            Err(_) => {
                preview.edited.push(one.from.clone());
                continue;
            }
        };
        // A file that cannot be read is not taken for one still as this device
        // placed it: an edited file is the one thing the preview must not miss.
        let unedited = departure::unedited(roots, &source, &local)
            .await
            .unwrap_or(false);
        if !unedited {
            preview.edited.push(one.from.clone());
        } else if follows(&mappings, &one.from, &one.to) {
            preview.following += 1;
        } else {
            preview.leaving += 1;
            preview.leaving_bytes += source.size;
        }
    }
    Ok(())
}
