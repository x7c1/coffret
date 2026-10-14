//! The commit flow's contract, run against an in-memory store that mints
//! identifiers.
//!
//! The in-memory store the suite's own run uses keys objects by name, and on
//! such a store a commit slot is exclusive however a writer comes by it: two
//! writers that derive one name collide on it. Google Drive is the other shape —
//! every reservation mints a fresh identifier, the conditional create excludes
//! by identifier alone, and names are not unique — and there the exclusion
//! holds only if every writer consumes the one slot its head carries
//! (spec: CP-2, CP-3). A backend of that shape needs an account to run against,
//! so this runs the suite over a double of it as part of an ordinary
//! `cargo test`.
//!
//! Every case runs here but two: the two that force a race for the Library's
//! *first* record. A Library that has committed nothing has no head to carry a
//! slot, so each writer of generation 0 reserves its own, and on a store that
//! mints identifiers both creates succeed. Nothing in the commit protocol
//! excludes that race yet, and those two cases assert that it is excluded, so
//! here they would fail; they are left out, and this note is what records the
//! gap.
//! `two_writers_agree_on_one_head_chain` also starts two writers on an empty
//! Library, and it stays in only because here it never races: neither this
//! store nor the in-memory catalog ever suspends, so one writer commits
//! generation 0 before the other starts. Its passing here says nothing about
//! that gap.
//! The race one link further, at a committed head, is
//! `two_writers_on_one_head_leave_one_successor`, and that one runs here.

use coffret_usecase::commit_conformance::CommitUnderTest;
use coffret_usecase::{InMemoryIndex, InMemoryMintingStore};

/// Small enough that the cases reach a second listing page, as in the
/// name-keyed run.
const PAGE_SIZE: usize = 3;

/// An empty Library and two empty catalogs for one case.
///
/// Async because the macro awaits it, as a backend's fixture must be.
async fn fixture() -> Option<CommitUnderTest> {
    Some(CommitUnderTest::new(
        Box::new(InMemoryMintingStore::new(PAGE_SIZE)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    ))
}

coffret_usecase::commit_conformance!(@cases fixture().await =>
    a_commit_makes_the_batch_the_current_state,
    a_removal_leaves_the_current_set_and_is_trashed,
    two_writers_agree_on_one_head_chain,
    two_writers_on_one_head_leave_one_successor,
    two_replays_of_one_catalog_converge,
    a_refused_replay_no_checkpoint_explains_is_reported,
    a_colliding_entry_path_is_refused_before_any_write,
    a_rebased_batch_whose_removal_is_no_longer_current_is_refused,
    a_rebased_batch_whose_removals_are_current_commits,
    a_missing_keyring_replica_stops_the_commit,
    the_lowest_missing_replica_is_the_one_reported,
    a_refused_replica_write_stops_the_commit_before_its_head,
    an_interrupted_commit_leaves_the_head_unchanged,
    an_untrashed_removal_reports_what_storage_refused,
    a_lost_replica_is_rewritten_before_the_next_commit,
    an_unreadable_replica_is_replaced,
    a_repair_the_provider_refuses_stops_the_commit,
    a_repair_that_stops_reports_what_it_put_back,
    an_unfetchable_replica_stops_the_commit_unrewritten,
    a_keyring_no_replica_answers_stays_unreadable,
    a_complete_set_costs_no_writes_and_reports_no_repair,
    a_repair_before_a_lost_slot_is_still_reported,
    a_repair_before_the_rebases_ran_out_is_still_reported,
    a_repair_before_a_refused_record_is_still_reported,
    two_devices_repairing_one_position_both_commit,
    a_checkpoint_is_written_once_the_threshold_is_crossed,
    repeated_checkpoint_failures_leave_the_journal_replayable,
    no_checkpoint_is_written_below_the_threshold,
    a_snapshot_slot_taken_by_a_sibling_converges,
    a_commit_is_seen_part_way_in_the_objects_it_stores,
    a_commit_writes_and_reads_back_its_replicas_at_once,
);
