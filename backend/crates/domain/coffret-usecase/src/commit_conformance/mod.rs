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
//! Four of the cases need Storage to misbehave — a replica that never arrives,
//! a head that refuses the create, a snapshot slot a sibling reached first, a
//! provider that will not move anything to the trash — and reach it by wrapping
//! whatever store the backend handed over. That keeps them backend-agnostic: the
//! same fault runs against a real provider and in memory.
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

mod checkpoint;
pub use checkpoint::{
    a_checkpoint_is_written_once_the_threshold_is_crossed,
    a_snapshot_slot_taken_by_a_sibling_converges, no_checkpoint_is_written_below_the_threshold,
};

mod commit_under_test;
pub use commit_under_test::CommitUnderTest;

mod faulty_store;

mod fixtures;

mod happy_path;
pub use happy_path::{
    a_commit_makes_the_batch_the_current_state, a_removal_leaves_the_current_set_and_is_trashed,
};

mod library;

mod race;
pub use race::{
    a_refused_replay_no_checkpoint_explains_is_reported,
    a_writer_that_loses_the_slot_rebases_onto_the_new_head, two_replays_of_one_catalog_converge,
    two_writers_settle_on_one_head_chain,
};

mod racing_store;

mod rival_index;

mod refusals;
pub use refusals::{
    a_colliding_entry_path_is_refused_before_any_write, a_missing_keyring_replica_stops_the_commit,
    an_interrupted_commit_leaves_the_head_unchanged,
    an_untrashed_removal_reports_what_storage_refused,
};

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

/// Declares the whole commit conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`CommitUnderTest`]`>`: `Some` with an empty store and
/// two empty catalogs to run the case against, or `None` to skip it because this
/// backend is not configured in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::commit_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! commit_conformance {
    ($setup:expr) => {
        $crate::commit_conformance!(@cases $setup =>
            a_commit_makes_the_batch_the_current_state,
            a_removal_leaves_the_current_set_and_is_trashed,
            two_writers_settle_on_one_head_chain,
            a_writer_that_loses_the_slot_rebases_onto_the_new_head,
            two_replays_of_one_catalog_converge,
            a_refused_replay_no_checkpoint_explains_is_reported,
            a_colliding_entry_path_is_refused_before_any_write,
            a_missing_keyring_replica_stops_the_commit,
            an_interrupted_commit_leaves_the_head_unchanged,
            an_untrashed_removal_reports_what_storage_refused,
            a_checkpoint_is_written_once_the_threshold_is_crossed,
            no_checkpoint_is_written_below_the_threshold,
            a_snapshot_slot_taken_by_a_sibling_converges,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::commit_conformance::$case(&fixture).await,
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
