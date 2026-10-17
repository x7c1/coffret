//! Applying a Journal record's `renames` (spec: FM-15, RV-5).
//!
//! A rename changes an Entry's current Entry Path in the catalog and nothing
//! else: the Entry keeps its Container, its entry number, its extent, and the
//! captured path its Container recorded. A record's removals, renames and
//! additions are one change — removals out, then every rename at once, then
//! additions in — and a rename the catalog does not admit refuses the whole of
//! it, leaving the catalog and this device's own rows exactly as they were.

use coffret_model::{EntryLocation, EntryNumber, EntryRename, JournalRecord};

use crate::index::Index;
use crate::index_conformance::fixtures::{container_id, path};

// What a record's renames do to the catalog, which renames the catalog
// refuses, how this device's files of the renamed Entries follow them, and how
// a record the catalog already stands past is refused before any of that — a
// module to each, beside the helpers they share.
mod already_applied;
pub use already_applied::a_record_below_the_checkpoint_is_refused_and_changes_nothing;

mod applied;
pub use applied::{
    a_containers_entries_are_listed_by_number_at_their_current_paths,
    a_folders_worth_of_renames_lands_in_one_record, a_rename_changes_one_files_current_path,
    a_rename_inside_a_pack_leaves_its_neighbours,
    a_replay_with_renames_reaches_what_a_restore_and_the_rest_would,
    the_captured_path_stands_apart_exactly_where_a_rename_leaves_it,
    two_entries_swap_their_names_in_one_record,
};

mod refused;
pub use refused::{
    a_rename_in_a_container_that_is_not_current_is_refused,
    a_rename_in_a_container_the_record_removes_is_refused, a_rename_of_a_derived_entry_is_refused,
    a_rename_onto_a_standing_entry_is_refused, a_rename_onto_the_records_own_addition_is_refused,
    a_rename_past_the_entry_table_is_refused, a_rename_that_changes_nothing_is_refused,
    two_renames_onto_one_path_are_refused,
};

mod relocation;
pub use relocation::{
    a_restore_past_a_rename_marks_the_file_to_follow_it,
    a_restore_past_the_removal_forgets_the_file_under_a_renamed_entry,
    the_file_of_a_removed_entry_is_forgotten_under_a_renamed_one,
    the_file_of_a_removed_entry_stops_waiting_for_it,
    the_file_of_a_renamed_entry_waits_to_follow_it,
};

/// The rename of the Entry numbered `number` in the Container of `seed` to
/// `to`.
fn rename(seed: u8, number: u32, to: &str) -> EntryRename {
    EntryRename::new(container_id(seed), entry_number(number), path(to))
        .expect("a fixture renames into an ordinary path")
}

fn entry_number(number: u32) -> EntryNumber {
    EntryNumber::new(u64::from(number)).expect("a fixture number is a u32")
}

/// `record` renaming `renames` as well, given in the order FM-15 fixes.
fn renaming(record: JournalRecord, renames: Vec<EntryRename>) -> JournalRecord {
    record
        .with_renames(renames)
        .expect("a fixture lists its renames in canonical order")
}

/// The Entry at `text`, which the case expects to be there.
async fn at(index: &dyn Index, text: &str) -> EntryLocation {
    index
        .entry_at(&path(text))
        .await
        .expect("looking a path up must succeed")
        .unwrap_or_else(|| panic!("an Entry stands at {text}"))
}

/// Whether any Entry stands at `text`.
async fn holds(index: &dyn Index, text: &str) -> bool {
    index
        .entry_at(&path(text))
        .await
        .expect("looking a path up must succeed")
        .is_some()
}

/// The current and captured path of every Entry, in Entry Path order.
async fn names(index: &dyn Index) -> Vec<(String, String)> {
    index
        .entries_under(None)
        .await
        .expect("listing the catalog must succeed")
        .into_iter()
        .map(|location| {
            (
                location.entry.path.as_str().to_owned(),
                location.entry.captured_path.as_str().to_owned(),
            )
        })
        .collect()
}

/// Where the file at `text` waits to follow its Entry to, if it does.
async fn relocate_to(index: &dyn Index, text: &str) -> Option<String> {
    index
        .local_entry_at(&path(text))
        .await
        .expect("reading a local file's row must succeed")
        .unwrap_or_else(|| panic!("this device keeps a row for {text}"))
        .relocate_to
        .map(|to| to.as_str().to_owned())
}
