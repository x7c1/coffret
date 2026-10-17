//! The commit flow's contract, as tests.
//!
//! [`conformance`](crate::conformance) holds each adapter to what an
//! `ObjectStore` means and [`index_conformance`](mod@crate::index_conformance) to
//! what an `Index` means. Neither says anything about what happens when a commit
//! drives both of them at once, and that is where the Library's correctness
//! actually lives: a store whose conditional create is exclusive and a catalog
//! that replays records faithfully still leave open whether two devices
//! committing at the same moment end up with one head chain.
//!
//! So this is a third suite, over the pair. Each case takes a
//! [`CommitUnderTest`] — one store and two catalogs — and drives
//! [`commit_batch`](crate::commit::commit_batch) against it, except the two
//! about a catalog with a second replayer over it, which drive the `catch_up`
//! every commit begins with; [`commit_conformance!`](crate::commit_conformance!)
//! turns the whole set into ordinary `#[tokio::test]` functions in a backend's
//! test target.
//!
//! What the cases assert is deliberately not what the call returned. They read
//! Storage back the way a device with no Index would — through the format layer,
//! under keys derived independently of the flow — because a commit is worth
//! exactly what another device can find afterwards: a record that decodes to the
//! batch (spec: FM-15), a Keyring set that is complete and valid under the tuple
//! that record names (spec: CP-10, KL-1, KL-2), and a checkpoint under the one
//! name its head gives it (spec: CK-10).
//!
//! Ten of the cases need Storage to misbehave, in six ways — a replica that
//! never arrives, a head that refuses the create, a snapshot slot a sibling
//! reached first, a provider that will not move anything to the trash, one that
//! refuses a replica write, one that holds a replica and will not hand it
//! over — and reach it by wrapping whatever store the backend handed over. That
//! keeps them backend-agnostic: the same fault runs against a real provider and
//! in memory.
//!
//! Five of the repair cases wrap that store to *watch* rather than to
//! misbehave, two of them around one of those faults, because what they assert
//! is which objects reached Storage: a repair rewrites the positions the
//! committed set has lost and no others, a complete set costs no write at all,
//! and a repair that may not go ahead writes over nothing (spec: KL-13, KL-16).
//!
//! Seven other cases wrap that store to let a rival device commit first, at the
//! exact moment the writer under test reaches the create of its record. Two
//! commits merely started together collide or do not depending on how the
//! runtime interleaves them, and a suite that started two writers at once
//! would pass either way; this puts the collision where it can be asserted.
//!
//! One case wraps that store to hold each Keyring replica request until the
//! whole set is in flight, because what it asserts is that a commit sends its
//! replicas together: a commit that sent them one after another would land all
//! the same, only slower, and nothing it returns would say so.
//!
//! Two progress cases wrap that store to note what the run had said each time
//! a commit sent it an object, because a commit that reported every step at
//! the end would record the same list as one that reported each as it went.
//!
//! The two catch-up cases wrap the catalog instead, for the same reason: a
//! second replayer over one Index is another process rather than a fault, and
//! putting it inside the call under test is what makes it happen on every
//! backend rather than on the slow ones. What they read back afterwards is that
//! catalog, since what a catch-up is worth is what it left standing there.
//!
//! The module lives in the domain crate, next to the flow it is the contract
//! for, and does no I/O of its own. It is behind the `conformance` feature so
//! that only test targets pay for it.

use coffret_model::ControlObjectName;

#[cfg(test)]
mod carried_slot_tests;

#[cfg(test)]
mod first_head_tests;

mod checkpoint_failure;
mod refused_checkpoint;
pub use checkpoint_failure::repeated_checkpoint_failures_leave_the_journal_replayable;

mod checkpoint;
pub use checkpoint::{
    a_checkpoint_is_written_once_the_threshold_is_crossed,
    a_snapshot_slot_taken_by_a_sibling_converges, no_checkpoint_is_written_below_the_threshold,
};

mod commit_under_test;
pub use commit_under_test::CommitUnderTest;

mod counting_store;

// Visible to the sync suite, which borrows it: a settle trashes an abandoned
// Container the way a commit trashes a removed one, and a provider that refuses
// the one refuses the other.
pub(crate) mod faulty_store;

mod fixtures;

mod gathering_store;

mod happy_path;
pub use happy_path::{
    a_commit_makes_the_batch_the_current_state, a_removal_leaves_the_current_set_and_is_trashed,
};

mod library;

mod progress;
pub use progress::{
    a_commit_is_seen_part_way_in_the_objects_it_stores,
    a_commit_that_rebases_counts_again_from_nothing,
};

mod race;
pub use race::{
    a_refused_replay_no_checkpoint_explains_is_reported,
    a_writer_that_loses_the_slot_rebases_onto_the_new_head, two_replays_of_one_catalog_converge,
    two_writers_agree_on_one_head_chain, two_writers_on_one_head_leave_one_successor,
};

mod racing_store;

#[cfg(test)]
mod rebase_tests;

mod refusals;
pub use refusals::{
    a_colliding_entry_path_is_refused_before_any_write, a_missing_keyring_replica_stops_the_commit,
    a_refused_replica_write_stops_the_commit_before_its_head,
    an_interrupted_commit_leaves_the_head_unchanged,
    an_untrashed_removal_reports_what_storage_refused,
    the_lowest_missing_replica_is_the_one_reported,
};

mod repair;
pub use repair::{
    a_complete_set_costs_no_writes_and_reports_no_repair,
    a_keyring_no_replica_answers_stays_unreadable,
    a_lost_replica_is_rewritten_before_the_next_commit,
    a_repair_before_a_lost_slot_is_still_reported,
    a_repair_before_a_refused_record_is_still_reported,
    a_repair_before_the_rebases_ran_out_is_still_reported,
    a_repair_that_stops_reports_what_it_put_back, a_repair_the_provider_refuses_stops_the_commit,
    an_unfetchable_replica_stops_the_commit_unrewritten, an_unreadable_replica_is_replaced,
    two_devices_repairing_one_position_both_commit,
};

mod replica_overlap;
pub use replica_overlap::a_commit_writes_and_reads_back_its_replicas_at_once;

mod rival_index;

mod stale_removal;
pub use stale_removal::{
    a_rebased_batch_whose_removal_is_no_longer_current_is_refused,
    a_rebased_batch_whose_removals_are_current_commits,
};

mod watching_store;

/// Whether a name is a link in the control-head chain (spec: FM-12).
///
/// Both store wrappers act at the moment a commit reaches the create of its
/// record, and that moment is exactly this name — one refusing it, the other
/// letting a rival in first — so the predicate is shared rather than written
/// once per wrapper.
fn is_head(name: &str) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::Head { .. })
    )
}

/// Creates the Library a case runs in, in the empty store the backend handed
/// over: its first head, written as creating a Library on a device writes it
/// (spec: CP-2, FM-13).
///
/// Every case starts here, so what each asserts is what a commit does to a
/// Library as it exists once created — whose first commit consumes the slot
/// `head-0` carries, like every commit after it. Public only for
/// [`commit_conformance!`](crate::commit_conformance!), which calls it before
/// each case.
#[doc(hidden)]
pub async fn create_library(fixture: &CommitUnderTest) {
    create_library_in(
        fixture.store(),
        &fixtures::control_keys(),
        &fixtures::policy(),
    )
    .await;
}

/// Writes the first head of a Library into an empty store, as creating the
/// Library on a device does (spec: CP-2, FM-13).
///
/// Shared by every suite whose cases commit, so that each of them starts from a
/// Library as it exists once created rather than from one no device makes any
/// more.
pub(crate) async fn create_library_in(
    store: &dyn crate::object_store::ObjectStore,
    keys: &crate::commit::ControlKeys,
    policy: &crate::commit::CommitPolicy,
) {
    crate::commit::write_first_head(store, keys, policy)
        .await
        .expect("creating a Library in an empty store must succeed")
        .expect("an empty store holds no head to find instead");
}

/// Declares the whole commit conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`CommitUnderTest`]`>`: `Some` with an empty store and
/// two empty catalogs to run the case against, or `None` to skip it because this
/// backend is not configured in this environment. Each case first creates the
/// Library in that store ([`create_library`]).
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::commit_conformance!(my_fixture().await);
/// ```
///
/// The `@cases` form takes the same argument followed by `=>` and a list of
/// case names, and declares only those. It is for a run that cannot hold every
/// case; a case added to the suite is not in such a run until it is added to
/// that list as well.
///
/// ```ignore
/// coffret_usecase::commit_conformance!(@cases my_fixture().await =>
///     a_commit_makes_the_batch_the_current_state,
///     two_writers_on_one_head_leave_one_successor,
/// );
/// ```
#[macro_export]
macro_rules! commit_conformance {
    ($setup:expr) => {
        $crate::commit_conformance!(@cases $setup =>
            a_commit_makes_the_batch_the_current_state,
            a_removal_leaves_the_current_set_and_is_trashed,
            two_writers_agree_on_one_head_chain,
            a_writer_that_loses_the_slot_rebases_onto_the_new_head,
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
            a_commit_that_rebases_counts_again_from_nothing,
            a_commit_writes_and_reads_back_its_replicas_at_once,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => {
                        $crate::commit_conformance::create_library(&fixture).await;
                        $crate::commit_conformance::$case(&fixture).await
                    }
                    None => eprintln!(
                        concat!(
                            "skipping ",
                            stringify!($case),
                            ": no Library is configured in this environment",
                        ),
                    ),
                }
            }
        )+
    };
}
