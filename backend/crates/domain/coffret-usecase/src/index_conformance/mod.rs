//! The [`Index`](crate::Index) contract, as tests.
//!
//! The Index caches the shared Catalog beside device-local records that
//! Storage cannot reconstruct. Every implementation must preserve that
//! boundary. Its cached state answers "which Container holds this",
//! and two devices whose catalogs disagree write Snapshots that disagree — so
//! the contract is one suite every implementation runs rather than one suite
//! per implementation.
//!
//! Each case takes an [`IndexUnderTest`] and asserts one piece of the contract;
//! [`index_conformance!`](crate::index_conformance!) turns the whole set into
//! ordinary `#[tokio::test]` functions in an adapter's test target.
//!
//! What the cases are really checking is one property in several shapes: the
//! cached Catalog is a pure function of the control state
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
    a_batch_is_marked_attempted_together,
    a_batch_keeps_every_slot_it_attempted_until_its_last_row_goes,
    a_file_left_behind_by_the_library_is_reported,
    a_forgotten_row_is_gone_and_forgetting_twice_succeeds,
    a_held_parcel_is_recorded_until_it_is_let_go, a_mapping_is_kept_once_per_prefix,
    a_mapping_round_trips_its_root_identity, a_materialization_keeps_the_hash_it_was_recorded_with,
    a_refresh_releases_the_reservations_its_batch_packs, a_replay_leaves_device_state_alone,
    a_reservation_is_kept_in_order_until_it_is_released, a_restore_leaves_device_state_alone,
    a_spool_is_recorded_until_its_row_is_cleared,
    a_spooling_row_becomes_spooled_when_its_file_completes,
    a_spooling_row_that_names_an_object_is_refused, only_a_file_this_device_had_can_go_absent,
    the_entries_absent_under_a_prefix_are_the_ones_not_on_disk,
};

mod fixtures;

mod identity;
pub use identity::{
    a_departed_file_is_forgotten_where_a_rename_brings_another_entry,
    a_record_of_an_entry_standing_elsewhere_waits_for_it, a_relocated_record_follows_its_file,
    a_restore_carries_a_record_over_to_a_replacement,
    a_restore_past_a_swap_and_a_removal_claims_neither_entry,
    an_addition_after_a_removal_is_not_the_departed_files_entry,
};

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
    a_refused_operation_leaves_the_whole_catalog_as_it_was, one_container_added_twice_is_refused,
    two_entries_at_one_path_are_refused,
};

mod renames;
pub use renames::{
    a_containers_entries_are_listed_by_number_at_their_current_paths,
    a_deposit_time_survives_a_replay_a_rename_and_a_restore,
    a_folders_worth_of_renames_lands_in_one_record,
    a_record_below_the_checkpoint_is_refused_and_changes_nothing,
    a_rename_changes_one_files_current_path,
    a_rename_in_a_container_that_is_not_current_is_refused,
    a_rename_in_a_container_the_record_removes_is_refused,
    a_rename_inside_a_pack_leaves_its_neighbours, a_rename_of_a_derived_entry_is_refused,
    a_rename_onto_a_standing_entry_is_refused, a_rename_onto_the_records_own_addition_is_refused,
    a_rename_past_the_entry_table_is_refused, a_rename_that_changes_nothing_is_refused,
    a_replay_with_renames_reaches_what_a_restore_and_the_rest_would,
    a_restore_past_a_rename_marks_the_file_to_follow_it,
    a_restore_past_the_removal_forgets_the_file_under_a_renamed_entry,
    the_captured_path_stands_apart_exactly_where_a_rename_leaves_it,
    the_file_of_a_removed_entry_is_forgotten_under_a_renamed_one,
    the_file_of_a_removed_entry_stops_waiting_for_it,
    the_file_of_a_renamed_entry_waits_to_follow_it, two_entries_swap_their_names_in_one_record,
    two_renames_onto_one_path_are_refused,
};

mod replay;
pub use replay::{
    a_birth_time_survives_a_replay_and_a_query, a_captured_path_survives_a_replay_and_a_restore,
    a_fresh_index_stands_at_no_committed_state, a_replay_reaches_what_a_restore_of_the_head_would,
    a_restore_replaces_the_whole_catalog, a_restore_round_trips_through_a_checkpoint,
    an_entry_number_survives_a_replay_and_a_restore,
    removing_a_container_removes_the_entries_it_held,
};

mod stored_form;
pub use stored_form::StoredForm;

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
            a_captured_path_survives_a_replay_and_a_restore,
            an_entry_number_survives_a_replay_and_a_restore,
            a_deposit_time_survives_a_replay_a_rename_and_a_restore,
            removing_a_container_removes_the_entries_it_held,
            a_refresh_lands_where_a_replay_of_its_record_does,
            a_refresh_marks_its_files_present_and_clears_its_spools,
            a_restore_leaves_device_state_alone,
            a_replay_leaves_device_state_alone,
            a_file_left_behind_by_the_library_is_reported,
            only_a_file_this_device_had_can_go_absent,
            the_entries_absent_under_a_prefix_are_the_ones_not_on_disk,
            a_materialization_keeps_the_hash_it_was_recorded_with,
            a_forgotten_row_is_gone_and_forgetting_twice_succeeds,
            a_mapping_is_kept_once_per_prefix,
            a_mapping_round_trips_its_root_identity,
            a_spool_is_recorded_until_its_row_is_cleared,
            a_spooling_row_becomes_spooled_when_its_file_completes,
            a_spooling_row_that_names_an_object_is_refused,
            a_batch_is_marked_attempted_together,
            a_batch_keeps_every_slot_it_attempted_until_its_last_row_goes,
            a_held_parcel_is_recorded_until_it_is_let_go,
            a_reservation_is_kept_in_order_until_it_is_released,
            a_refresh_releases_the_reservations_its_batch_packs,
            case_distinguishes_two_entry_paths,
            width_variants_are_two_entry_paths,
            a_prefix_covers_a_subtree_and_stops_at_the_separator,
            the_containers_under_a_prefix_are_reported_once,
            a_prefix_reports_only_what_this_device_materialized,
            two_entries_at_one_path_are_refused,
            one_container_added_twice_is_refused,
            a_record_already_applied_is_refused_and_the_checkpoint_stands,
            a_record_below_the_checkpoint_is_refused_and_changes_nothing,
            a_refused_operation_leaves_the_whole_catalog_as_it_was,
            a_rename_changes_one_files_current_path,
            a_rename_inside_a_pack_leaves_its_neighbours,
            a_folders_worth_of_renames_lands_in_one_record,
            two_entries_swap_their_names_in_one_record,
            a_containers_entries_are_listed_by_number_at_their_current_paths,
            the_captured_path_stands_apart_exactly_where_a_rename_leaves_it,
            a_replay_with_renames_reaches_what_a_restore_and_the_rest_would,
            the_file_of_a_renamed_entry_waits_to_follow_it,
            the_file_of_a_removed_entry_stops_waiting_for_it,
            the_file_of_a_removed_entry_is_forgotten_under_a_renamed_one,
            a_restore_past_a_rename_marks_the_file_to_follow_it,
            a_restore_past_the_removal_forgets_the_file_under_a_renamed_entry,
            a_rename_in_a_container_that_is_not_current_is_refused,
            a_rename_in_a_container_the_record_removes_is_refused,
            a_rename_past_the_entry_table_is_refused,
            a_rename_that_changes_nothing_is_refused,
            a_rename_of_a_derived_entry_is_refused,
            a_rename_onto_a_standing_entry_is_refused,
            two_renames_onto_one_path_are_refused,
            a_rename_onto_the_records_own_addition_is_refused,
            a_departed_file_is_forgotten_where_a_rename_brings_another_entry,
            an_addition_after_a_removal_is_not_the_departed_files_entry,
            a_restore_past_a_swap_and_a_removal_claims_neither_entry,
            a_restore_carries_a_record_over_to_a_replacement,
            a_record_of_an_entry_standing_elsewhere_waits_for_it,
            a_relocated_record_follows_its_file,
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
