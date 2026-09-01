//! The [`Index`](crate::Index) contract, as tests.
//!
//! The Index is a cache that no user data depends on, which makes it tempting
//! to let each implementation mean something slightly different by it. It is
//! also what every device's answer to "which Container holds this" comes from,
//! and two devices whose catalogs disagree write Snapshots that disagree — so
//! the contract is one suite every implementation runs rather than one suite
//! per implementation.
//!
//! Each case takes an [`IndexUnderTest`] and asserts one piece of the contract;
//! [`index_conformance!`](crate::index_conformance!) turns the whole set into
//! ordinary `#[tokio::test]` functions in an adapter's test target.
//!
//! What the cases are really checking is one property in several shapes: the
//! Library-wide half of a catalog is a pure function of the control state
//! applied to it, and the device-local half is untouched by that control state.
//! Replay against restore, this device's commit against another's replay of it,
//! and device state across both are the three faces of it.
//!
//! The module lives in the domain crate, next to the trait it is the contract
//! for, and touches no file of its own: it drives whatever catalog an adapter
//! hands it. It is behind the `conformance` feature so that only test targets
//! pay for it.

mod commit;
pub use commit::{
    a_refresh_lands_where_a_replay_of_its_record_does,
    a_refresh_marks_its_files_present_and_clears_its_spools,
};

mod device_state;
pub use device_state::{
    a_file_left_behind_by_the_library_is_reported, a_mapping_is_kept_once_per_prefix,
    a_mapping_round_trips_its_root_identity, a_replay_leaves_device_state_alone,
    a_restore_leaves_device_state_alone, a_spool_is_recorded_until_its_batch_settles,
    a_spooling_row_becomes_spooled_when_its_file_completes,
    only_a_file_this_device_had_can_go_absent,
};

mod fixtures;

mod index_under_test;
pub use index_under_test::IndexUnderTest;

mod paths;
pub use paths::{
    a_prefix_covers_a_subtree_and_stops_at_the_separator,
    a_prefix_reports_only_what_this_device_materialized, case_distinguishes_two_entry_paths,
    the_containers_under_a_prefix_are_reported_once, width_variants_are_two_entry_paths,
};

mod refusals;
pub use refusals::{
    a_record_already_applied_is_refused_and_the_checkpoint_stands,
    a_refused_operation_leaves_the_whole_catalog_as_it_was,
    an_entry_without_its_container_is_refused, one_container_added_twice_is_refused,
    two_entries_at_one_path_are_refused,
};

mod replay;
pub use replay::{
    a_birth_time_survives_a_replay_and_a_query, a_fresh_index_stands_at_no_committed_state,
    a_replay_reaches_what_a_restore_of_the_head_would, a_restore_replaces_the_whole_catalog,
    a_restore_round_trips_through_a_checkpoint, removing_a_container_removes_the_entries_it_held,
};

/// Declares the whole Index conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`IndexUnderTest`]`>`: `Some` with two empty,
/// independent catalogs to run the case against, or `None` to skip it because
/// this implementation cannot be reached in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among
/// its dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::index_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! index_conformance {
    ($setup:expr) => {
        $crate::index_conformance!(@cases $setup =>
            a_fresh_index_stands_at_no_committed_state,
            a_restore_round_trips_through_a_checkpoint,
            a_restore_replaces_the_whole_catalog,
            a_replay_reaches_what_a_restore_of_the_head_would,
            a_birth_time_survives_a_replay_and_a_query,
            removing_a_container_removes_the_entries_it_held,
            a_refresh_lands_where_a_replay_of_its_record_does,
            a_refresh_marks_its_files_present_and_clears_its_spools,
            a_restore_leaves_device_state_alone,
            a_replay_leaves_device_state_alone,
            a_file_left_behind_by_the_library_is_reported,
            only_a_file_this_device_had_can_go_absent,
            a_mapping_is_kept_once_per_prefix,
            a_mapping_round_trips_its_root_identity,
            a_spool_is_recorded_until_its_batch_settles,
            a_spooling_row_becomes_spooled_when_its_file_completes,
            case_distinguishes_two_entry_paths,
            width_variants_are_two_entry_paths,
            a_prefix_covers_a_subtree_and_stops_at_the_separator,
            the_containers_under_a_prefix_are_reported_once,
            a_prefix_reports_only_what_this_device_materialized,
            two_entries_at_one_path_are_refused,
            one_container_added_twice_is_refused,
            an_entry_without_its_container_is_refused,
            a_record_already_applied_is_refused_and_the_checkpoint_stands,
            a_refused_operation_leaves_the_whole_catalog_as_it_was,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::index_conformance::$case(&fixture).await,
                    None => eprintln!(
                        concat!(
                            "skipping ",
                            stringify!($case),
                            ": no Index is configured in this environment",
                        ),
                    ),
                }
            }
        )+
    };
}
