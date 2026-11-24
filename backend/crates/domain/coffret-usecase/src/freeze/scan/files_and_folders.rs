use coffret_model::{ContainerId, EntryPath};

use crate::device_state::Mapping;
use crate::file_and_folder::{refuse_additions, Adding};
use crate::freeze::file_and_folder::FileAndFolder;
use crate::freeze::freeze_error::FreezeResult;
use crate::index::Index;

/// Every file of a selection whose addition would make one Entry Path both a
/// file and a folder, in Entry Path order (spec: EP-4, EP-6).
///
/// `selection` is each file a freeze would pack, with the one-file Container
/// it absorbs, in Entry Path order. The rule is asked as the commit would ask
/// it — against the catalog the caller has read, with those Containers gone as
/// the commit removes them, and the files held to it against one another in
/// Entry Path order, so that of two that cannot both be added the later is the
/// one named. The run and the preview both ask it here, so the two cannot
/// disagree about which files stand in the way.
pub(super) async fn files_and_folders(
    index: &dyn Index,
    selection: &[(&EntryPath, Option<ContainerId>)],
) -> FreezeResult<Vec<FileAndFolder>> {
    let adding: Vec<Adding<'_>> = selection
        .iter()
        .map(|&(path, removes)| Adding { path, removes })
        .collect();
    let refusals = refuse_additions(index, &adding, &[]).await?;
    if refusals.iter().all(Option::is_none) {
        return Ok(Vec::new());
    }
    // Against every mapping this device records, available or not, as the
    // partition of the Library's namespace is read (spec: EP-9).
    let mappings = index.mappings().await?;
    let mapped = Mapping::represented_prefixes(&mappings);
    Ok(selection
        .iter()
        .zip(refusals)
        .filter_map(|(&(path, _), refusal)| {
            refusal.map(|both| FileAndFolder {
                path: path.clone(),
                mapped: mapped.contains(both.as_str()),
                both,
            })
        })
        .collect())
}
