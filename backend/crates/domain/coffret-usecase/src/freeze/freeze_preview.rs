use std::collections::BTreeSet;

use coffret_model::EntryPath;

use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::scan::{self, Scope};
use crate::index::Index;
use crate::local_scan::UnknownBirths;
use crate::mapped_roots::MappedRoots;
use crate::unavailable_root::UnavailableRoot;

/// What a freeze of one folder would select on this device, counted before it
/// is asked for.
///
/// Every count is of files the run's own scan would consider, judged by the
/// run's own rules (spec: PK-1, PK-2, EP-10, PK-17), except
/// [`not_here`](Self::not_here), which also counts the Entries under the folder
/// whose files this device does not have at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FreezePreview {
    /// How many files a freeze would pack.
    pub files: usize,
    /// Their total length on disk, in bytes, as the walk read it.
    pub bytes: u64,
    /// How many files an existing Pack already holds, which a freeze leaves
    /// alone (spec: PK-1, PK-2).
    pub in_pack: usize,
    /// How many Pack-held files have moved on disk since this device last saw
    /// them. A freeze leaves them alone too, and reports the ones whose
    /// content changed (spec: PK-14); a preview does not read them to tell.
    pub changed_in_pack: usize,
    /// How many Entries under the folder this device has no file of its own
    /// for: none on disk, or one this device never placed (spec: EP-10).
    pub not_here: usize,
    /// The mappings representing the folder whose roots this device cannot
    /// vouch for — at most one, unless the folder is the Library root — under
    /// which nothing was counted, not even as not here (spec: EP-12).
    pub unavailable: Vec<UnavailableRoot>,
}

/// Counts what [`freeze_folder`](super::freeze_folder) would select under
/// `prefix` — and, where `only` names them, among exactly those Entry Paths —
/// without reading a file's content, reaching Storage, or writing anything.
///
/// It is the run's own scan stopped before the first file is hashed, so the
/// two cannot disagree about which files are selected; they can disagree only
/// where the folder or the Library changes between the preview and the run.
pub async fn preview_freeze(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    births: &UnknownBirths,
    prefix: Option<&EntryPath>,
    only: Option<&BTreeSet<EntryPath>>,
) -> FreezeResult<FreezePreview> {
    scan::preview(index, roots, births, Scope { prefix, only }).await
}
