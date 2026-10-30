//! What a sync does with the file of an Entry a rename gave another path
//! (spec: FM-15, EP-10).
//!
//! A rename changes an Entry's current path in the catalog, and the file this
//! device placed for it stays where it was, its record waiting to follow. A
//! sync relocates every such file before it scans: one rename inside the
//! mapped root, or across two on one filesystem, and the record after it. A
//! file that cannot follow at all — no mapped root reaches the new path, or
//! the one that does is on another filesystem — leaves this device: discarded
//! into the trash where it is unedited. A file that cannot be relocated this
//! run — its root not there, the file edited, something of the person's where
//! it would go — is left exactly where it stands, reported, and tried again by
//! the next run.
//! Throughout, nothing is uploaded, overwritten, or claimed for another Entry.

use std::path::{Path, PathBuf};

use coffret_format::Purpose;
use coffret_model::EntryRename;

use crate::commit::catch_up;
use crate::commit_conformance::commit_renames_in;
use crate::device_state::{LocalEntry, Mapping, RootMarkerId};
use crate::entry_paths::entry_path;
use crate::index::Index;
use crate::relocate::{Relocation, RelocationRefused};
use crate::sync::{sync_folders, SyncOutcome, SyncResult};
use crate::sync_conformance::fixtures::{keys, policy, purpose_key, request, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

// The shapes a relocation takes, what refuses one, what an interrupted one
// leaves for the next run, and the records a rename must not let claim another
// Entry — a module to each, beside the helpers they share.
mod identity;
pub use identity::{
    a_departed_file_is_not_claimed_by_an_entry_renamed_onto_its_path,
    a_snapshot_past_a_swap_and_a_removal_claims_neither_file,
};

mod interrupted;
pub use interrupted::{
    a_relocation_interrupted_after_its_rename_is_finished_by_the_next_run,
    a_relocation_interrupted_before_its_rename_is_made_by_the_next_run,
    an_exchange_interrupted_before_its_records_is_finished_by_the_next_run,
};

mod relocated;
pub use relocated::{
    a_file_already_placed_at_the_new_path_makes_the_waiting_one_redundant,
    a_folders_worth_of_pack_entries_is_relocated, a_rename_back_leaves_nothing_to_relocate,
    a_renamed_entrys_file_is_relocated_to_follow_it,
    files_whose_entries_traded_names_exchange_them,
};

mod leaving;
pub use leaving::{
    a_file_follows_its_entry_across_mapped_roots_on_one_filesystem,
    a_file_the_filesystem_will_not_rename_across_leaves_the_device,
    a_file_whose_new_path_no_mapping_reaches_leaves_the_device,
    a_file_whose_new_root_is_on_another_filesystem_leaves_the_device,
    an_edited_file_that_cannot_follow_is_kept_and_reported,
};

mod refusals;
pub use refusals::{
    a_cycle_refused_partway_reports_the_rest_as_waiting,
    a_file_behind_a_refused_waiting_file_waits_for_it,
    a_file_edited_since_it_was_placed_is_not_relocated,
    a_relocation_onto_a_name_the_system_files_rule_leaves_out_is_refused,
    a_relocation_through_a_symbolic_link_is_refused, a_relocation_under_an_unavailable_root_waits,
    a_relocation_without_an_atomic_rename_is_refused,
    something_standing_at_the_new_path_is_never_overwritten,
};

/// The identity the cases register their folder's root under (spec: EP-13),
/// so that a relocation is a write the run may make there.
fn registered() -> RootMarkerId {
    RootMarkerId::from_bytes([0x0e, 0x0a, 0x0e, 0x0d, 0x15, 0x0e, 0x01, 0x05])
}

/// Maps `root` onto the Library at `prefix` the way recording a mapping does:
/// a marker in the root, and the same identity expected of it.
async fn map_registered(fixture: &SyncUnderTest, prefix: Option<&str>, root: &Path) {
    fixture.fs().plant_marker(root, &registered());
    fixture
        .index()
        .set_mapping(
            Mapping::new(prefix.map(entry_path), root.to_path_buf()).expecting(registered()),
        )
        .await
        .expect("recording a mapping must succeed");
}

/// Maps the fixture's folder onto the Library root, writes `files` into it,
/// and carries them in with a first sync.
async fn carry_in(fixture: &SyncUnderTest, files: &[(&str, &[u8])]) {
    map_registered(fixture, None, fixture.folder()).await;
    for (relative, content) in files {
        write(fixture.fs(), fixture.folder(), relative, content);
    }
    let first = sync(fixture, 1).await;
    assert_eq!(first.added.len(), files.len(), "every file is carried in");
}

/// One sync run through `index`, as it came out.
async fn sync_through(
    fixture: &SyncUnderTest,
    index: &dyn Index,
    run: i64,
) -> SyncResult<SyncOutcome> {
    let keys = keys();
    sync_folders(request(fixture.store(), index, &keys, fixture.fs(), run)).await
}

/// One sync run, which the case expects to succeed.
async fn sync(fixture: &SyncUnderTest, run: i64) -> SyncOutcome {
    sync_through(fixture, fixture.index(), run)
        .await
        .unwrap_or_else(|error| panic!("sync run {run} must succeed: {error}"))
}

/// Brings this device's catalog to the head, as any run does first.
async fn catch_up_index(fixture: &SyncUnderTest, index: &dyn Index) {
    let keys = keys();
    catch_up(fixture.store(), index, keys.control(), &policy().retry)
        .await
        .unwrap_or_else(|error| panic!("catching up must succeed: {error}"));
}

/// Commits a record renaming the Entry at each `from` to its `to`, the way
/// another device's rename lands; this device's catalog has to stand at the
/// head, and does not apply the record itself.
async fn rename(fixture: &SyncUnderTest, renames: &[(&str, &str)]) {
    let mut named = Vec::with_capacity(renames.len());
    for (from, to) in renames {
        let entry = fixture
            .index()
            .entry_at(&entry_path(*from))
            .await
            .expect("looking a path up must succeed")
            .unwrap_or_else(|| panic!("an Entry stands at {from}"));
        named.push(
            EntryRename::new(entry.container_id, entry.entry_number, entry_path(*to))
                .expect("an ordinary path is a rename's target"),
        );
    }
    commit_renames_in(
        fixture.store(),
        fixture.index(),
        &purpose_key(Purpose::ControlJournal),
        named,
    )
    .await;
}

/// Every `(from, to)` a run relocated, sorted; panics on any other outcome.
fn relocated(outcome: &SyncOutcome) -> Vec<(String, String)> {
    let mut relocated: Vec<(String, String)> = outcome
        .relocations
        .iter()
        .map(|one| match one {
            Relocation::Relocated { from, to } => {
                (from.as_str().to_owned(), to.as_str().to_owned())
            }
            other => panic!("expected every waiting file relocated, got {other:?}"),
        })
        .collect();
    relocated.sort();
    relocated
}

/// The one refusal a run reported, as `(from, to, reason)`.
fn refused(outcome: &SyncOutcome) -> (String, String, RelocationRefused) {
    match outcome.relocations.as_slice() {
        [Relocation::Refused { from, to, reason }] => (
            from.as_str().to_owned(),
            to.as_str().to_owned(),
            reason.clone(),
        ),
        other => panic!("expected one refused relocation, got {other:?}"),
    }
}

/// Where `relative` stands in the fixture's folder.
fn at(fixture: &SyncUnderTest, relative: &str) -> PathBuf {
    fixture.folder().join(relative)
}

/// What the fixture's folder holds at `relative`, if anything.
fn content(fixture: &SyncUnderTest, relative: &str) -> Option<Vec<u8>> {
    fixture.fs().content(&at(fixture, relative))
}

/// This device's row for `text`, if it keeps one.
async fn row(fixture: &SyncUnderTest, text: &str) -> Option<LocalEntry> {
    fixture
        .index()
        .local_entry_at(&entry_path(text))
        .await
        .expect("reading a local file's row must succeed")
}

/// Asserts that a run uploaded and committed nothing.
fn uploaded_nothing(outcome: &SyncOutcome) {
    assert!(outcome.added.is_empty(), "nothing is carried in");
    assert!(outcome.replaced.is_empty(), "nothing is replaced");
    assert!(outcome.commit.is_none(), "nothing is committed");
}
