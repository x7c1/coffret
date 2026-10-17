use std::collections::BTreeMap;

use coffret_model::{EntryNumber, EntryPath, JournalRecord};

use crate::commit::commit_error::CommitResult;
use crate::commit::prepared_batch::PreparedBatch;
use crate::entry_ref::EntryRef;
use crate::index::Index;

mod check_contested;
use check_contested::check_contested;

mod follow_replaced;
use follow_replaced::follow_replaced;

mod take_over;
use take_over::take_over;

/// The batch as it commits onto the head the Index now stands at, given the
/// records committed after the head it was prepared against (spec: CP-4,
/// EP-7).
///
/// `intervening` is every one of those records, in generation order, and the
/// Index stands at the last of them. Their renames are applied to the batch's
/// pending changes by `(Container ID, entry number)`, under EP-7's numbered
/// rules:
///
/// - an Entry an addition takes the place of — the one an update replaces, one
///   a Pack absorbs or a rebuilt Pack keeps — is taken over at the path the
///   renames left it at. The addition's Entry stands there, and its captured
///   path stays the name it was prepared under, which is what its Container
///   records (rules 2 and 6; spec: FM-9, FM-15);
/// - a rename whose Entry's Container a record removed follows the Entry that
///   record added at the name the rename was prepared against, and conflicts
///   where there is none (rule 3); one whose Entry a record renamed conflicts
///   (rule 4);
/// - an addition whose path a record gave another Entry by a rename (rule 1),
///   and a rename whose destination a record filled by an addition or a rename
///   (rule 5), conflict where that Entry holds the path still.
///
/// A removal is carried over unchanged: it names its Container, whatever the
/// Entries in it are called now (rule 7). Nothing is remapped by prefix — a
/// rename is exactly the Entries it was prepared with, and an addition under a
/// folder another writer renamed stays where it was prepared (rule 8).
///
/// The batch is always rebased from what was prepared rather than from an
/// earlier attempt's result, so each attempt reads the whole stretch the batch
/// did not see. What is left for [`candidate::check`](super::candidate::check)
/// is everything that does not depend on what happened since: a removal no
/// longer current, a path two additions claim, a rename the head does not
/// admit (spec: CP-18, EP-6, FM-15).
///
/// The local file the batch materialized for an Entry it takes over at another
/// path stays at the path it was captured from, and what the batch records of
/// it names the Entry that takes over: the refresh records it there, waiting
/// to be relocated to the Entry's new path (spec: EP-10).
///
/// # Errors
///
/// [`CommitError::RebaseConflict`](crate::commit::CommitError::RebaseConflict)
/// naming the first pending change that cannot be carried over, the record it
/// meets and why, and whatever the Index fails with.
pub(super) async fn rebase(
    index: &dyn Index,
    prepared: &PreparedBatch,
    intervening: &[JournalRecord],
) -> CommitResult<PreparedBatch> {
    if intervening.is_empty() {
        return Ok(prepared.clone());
    }
    let mut batch = prepared.clone();
    follow_replaced(&mut batch, intervening)?;
    let renamed = final_paths(intervening);
    check_contested(index, &batch, &renamed, intervening).await?;
    take_over(&mut batch, &renamed)?;
    Ok(batch)
}

/// Where the intervening records leave each Entry they rename: the path the
/// last of them gives it.
fn final_paths(intervening: &[JournalRecord]) -> BTreeMap<EntryRef, &EntryPath> {
    intervening
        .iter()
        .flat_map(JournalRecord::renames)
        .map(|rename| (EntryRef::renamed_by(rename), rename.path()))
        .collect()
}

/// The entry number of the Entry at `position` in an addition's table.
fn number_at(position: usize) -> EntryNumber {
    EntryNumber::at_position(position)
        .expect("an addition's table never runs past what entry numbers reach")
}
