use tracing::debug;

use crate::file_and_folder::{refuse_additions, Adding};
use crate::index::Index;
use crate::sync::surfaced::Surfaced;
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

/// Leaves out every file whose addition would make one Entry Path both a file
/// and a folder, reports each, and keeps the rest (spec: EP-4, EP-6).
///
/// A commit refuses such an addition, and a refused commit holds back the whole
/// batch, so the run asks first, against the catalog it has just caught up and
/// with the batch's own changes applied as the commit applies them: the
/// one-file Containers its replacements supersede leave, its renames take
/// effect, and the files are held to the rule against one another in Entry
/// Path order — the first of two that cannot both be added is the one kept.
/// A file left out is neither encoded nor uploaded, and the rest of the run
/// carries on.
///
/// A file standing where a detected rename was refused is one of them like any
/// other: it is read as new once the rename is refused, and where the rename
/// was refused for making a path both a file and a folder, the addition would
/// make that path too (spec: EP-10).
pub(super) async fn leave_out_files_and_folders(
    index: &dyn Index,
    survey: &mut Survey,
) -> SyncResult<()> {
    let refusals = {
        let adding: Vec<Adding<'_>> = survey
            .candidates
            .iter()
            .map(|candidate| Adding {
                path: &candidate.source.path,
                removes: candidate
                    .replaces
                    .as_ref()
                    .map(|location| location.container_id),
            })
            .collect();
        let renames: Vec<_> = survey
            .renames
            .iter()
            .map(|rename| (&rename.from, &rename.path))
            .collect();
        refuse_additions(index, &adding, &renames).await?
    };
    let mut refusals = refusals.into_iter();
    let mut surfaced = Vec::new();
    survey
        .candidates
        .retain(|candidate| match refusals.next().flatten() {
            None => true,
            Some(both) => {
                surfaced.push(Surfaced::FileAndFolder {
                    path: candidate.source.path.clone(),
                    both,
                });
                false
            }
        });
    let left_out = surfaced.len();
    survey.surfaced.extend(surfaced);

    // A count only: an Entry Path may not reach a diagnostic event.
    debug!(
        left_out,
        "left out the files that would make a path both a file and a folder"
    );
    Ok(())
}
