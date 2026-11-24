use crate::file_and_folder::{refuse_additions, Adding};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::not_frozen::NotFrozen;
use crate::freeze::survey::Survey;
use crate::index::Index;

/// Leaves out of the selection every file whose addition would make one Entry
/// Path both a file and a folder, and reports each (spec: EP-4, EP-6).
///
/// A commit refuses such an addition, and a refused commit holds back the whole
/// batch, so the run asks first, against the catalog it has caught up, with
/// the one-file Containers the selection absorbs gone as the commit removes
/// them, and the files held to the rule against one another in Entry Path
/// order — the first of two that cannot both be added is the one kept.
pub(super) async fn leave_out_files_and_folders(
    index: &dyn Index,
    survey: &mut Survey,
) -> FreezeResult<()> {
    let refusals = {
        let adding: Vec<Adding<'_>> = survey
            .selected
            .iter()
            .map(|selected| Adding {
                path: &selected.source.path,
                removes: selected.absorbs,
            })
            .collect();
        refuse_additions(index, &adding, &[]).await?
    };
    let mut refusals = refusals.into_iter();
    let mut surfaced = Vec::new();
    survey
        .selected
        .retain(|selected| match refusals.next().flatten() {
            None => true,
            Some(both) => {
                surfaced.push(NotFrozen::FileAndFolder {
                    path: selected.source.path.clone(),
                    both,
                });
                false
            }
        });
    survey.surfaced.extend(surfaced);
    Ok(())
}
