//! The folder sync's contract, as tests.
//!
//! [`commit_conformance`](mod@crate::commit_conformance) holds a store and a
//! catalog to what a commit over both of them means. This is the suite over the
//! step before it — the one that turns a folder on a device into the batch a
//! commit takes — and it is a suite of its own because what it can get wrong is
//! not what a commit can. A commit that is exclusive, replayable, and complete
//! still leaves open whether a second sync of an untouched folder uploads
//! everything again, whether a sync over a catalog standing behind the
//! Library's head uploads what the Library already holds, whether a file that
//! was merely touched costs an upload, whether a run killed mid-batch leaves
//! two Entries or none, and whether the ciphertext that reached Storage is the
//! file that was on disk.
//!
//! Each case takes a [`SyncUnderTest`] — one store, one catalog, and a folder —
//! and drives [`sync_folders`](crate::sync::sync_folders) against it;
//! [`sync_conformance!`](crate::sync_conformance!) turns the whole set into
//! ordinary `#[tokio::test]` functions in a backend's test target. Where the
//! ciphertext waits is the fixture's own in-memory spool rather than the
//! backend's directory: what a spool means is one contract for every backend,
//! held by [`spool_conformance`](mod@crate::spool_conformance).
//!
//! What the round-trip case asserts is deliberately not what the call returned.
//! It fetches the Container back off Storage, opens the envelope the committed
//! Keyring maps it to under a purpose key derived from the Master Key alone,
//! decodes it, and compares the bytes with the file — because a sync is worth
//! exactly what another enrolled device can open afterwards.
//!
//! Four things have to be arranged in the middle of a run for the cases that
//! need them: Storage misreporting what it stored, the catalog refusing the
//! refresh that follows a commit, the catalog holding every spool announcement
//! to the ordering it promises and refusing to mark a spool `Spooled`, and a
//! run's requests to Storage being tallied. Each wraps whatever
//! the backend handed the suite rather than replacing it, which keeps the cases
//! backend-agnostic — the same fault and the same count happen against a real
//! provider and in memory — and, for the two refusals, keeps what the interrupted
//! run wrote down in the very catalog the next run reads.
//!
//! The module lives in the domain crate, next to the flow it is the contract
//! for. It reads and writes files — a sync starts at a folder — but every one
//! of them is in the in-memory disk the fixture makes rather than on a directory
//! the backend hands it. It is behind the `conformance` feature so that only
//! test targets pay for it.

mod completion;
pub use completion::{
    a_commit_whose_refresh_failed_is_completed_and_replaced,
    a_completed_container_marks_its_file_present, a_run_with_no_pending_rows_reads_the_head_once,
};

mod counting_store;

mod departed;
pub use departed::{
    a_departed_file_in_a_trash_folder_inside_the_root_is_not_carried_back,
    a_departed_file_with_no_recorded_hash_is_kept_once_it_changed,
    a_departed_path_whose_file_is_gone_is_forgotten,
    a_file_put_where_one_was_moved_to_the_trash_is_new,
    a_freeze_leaves_a_departed_file_to_the_sync, a_path_a_new_entry_arrived_at_is_not_departed,
    a_refused_move_to_the_trash_leaves_the_file_and_is_retried,
    a_touched_departed_file_with_its_recorded_content_is_unedited,
    an_edited_departed_file_is_kept_and_reported_every_run,
    an_unedited_file_another_device_deleted_goes_to_the_trash,
    an_unedited_file_this_device_deleted_goes_to_the_trash,
    nothing_is_moved_to_the_trash_through_a_refused_root,
    nothing_is_moved_to_the_trash_under_an_unavailable_root,
};

// Visible to the freeze suite, which borrows the one helper that arranges an
// identity mismatch: what an unmounted disk looks like to the guard is one
// account, not one per suite.
pub(crate) mod fixtures;

mod import;
pub use import::{
    a_first_sync_commits_every_file_and_they_decode, a_mapped_prefix_decides_where_a_file_lands,
    a_top_level_mapping_takes_its_subtree_from_the_root_mapping,
    a_walked_files_birth_time_reaches_the_record, an_nfd_local_name_becomes_an_nfc_entry_path,
};

mod integrity;
pub use integrity::a_provider_hash_mismatch_is_refused;

mod uncertain_commit;
pub use uncertain_commit::{
    a_live_producer_keeps_exclusive_ownership,
    a_lost_response_and_withheld_head_never_authorize_trash,
    an_uncertain_commit_retains_its_ciphertext_and_provenance,
    another_writers_head_does_not_prove_an_uncertain_attempt,
};

mod interruption;
mod withheld_commit;
pub use interruption::{
    a_row_precedes_the_first_byte_of_a_spool,
    a_spool_left_by_an_interrupted_run_converges_to_one_entry,
    a_spooling_row_whose_spool_was_never_created_is_disposed,
    a_stale_pending_row_is_dropped_with_its_spool,
    an_abandoned_container_storage_will_not_trash_is_left_in_storage,
    an_unfinished_spool_is_disposed_with_its_row,
    an_uploaded_but_uncommitted_container_converges_to_one_entry,
    an_uploaded_container_is_settled_by_the_next_run,
};

mod mangling_store;

mod modification;
pub use modification::{
    a_modified_file_replaces_its_one_file_container,
    a_pack_resident_change_is_surfaced_and_untouched,
};

mod progress;
pub use progress::a_run_says_which_phase_it_is_in_and_counts_the_ones_it_can;

pub(crate) mod refusing_index;

mod repeat;
pub use repeat::{
    a_touched_file_with_equal_content_commits_nothing, an_unchanged_second_sync_commits_nothing,
};

mod roots;
pub use roots::{
    a_mapping_recorded_afresh_clears_its_identity_and_reports_the_deletions,
    a_missing_mapped_root_is_reported_and_infers_no_deletion,
    a_renumbered_root_that_holds_files_is_restamped_and_scans_normally,
    an_emptied_folder_on_the_recorded_filesystem_still_reports_its_deletions,
    an_empty_root_on_another_filesystem_is_reported_and_infers_no_deletion,
    an_unavailable_top_level_mapping_holds_its_subtree_back_from_the_root_mapping,
};

mod scope;
pub use scope::{
    a_file_deleted_locally_is_surfaced_and_untouched,
    an_entry_this_device_never_materialized_is_left_alone,
};

mod staleness;
pub use staleness::sync_catches_up_before_scanning;

mod stale_removal;
pub use stale_removal::a_replacement_for_a_removed_container_is_refused;

mod sync_under_test;
pub use sync_under_test::SyncUnderTest;

mod unmapped;
pub use unmapped::a_device_that_maps_nothing_is_told_apart_from_a_folder_with_nothing_new;

// Visible to the freeze suite, which borrows it: the Pack spool step keeps the
// same ordering, and one account of what that means is enough.
pub(crate) mod watching_index;

/// Declares the whole sync conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`SyncUnderTest`]`>`: `Some` with an empty store, an
/// empty catalog, and an empty folder to run the case against, or `None` to skip
/// it because this backend is not configured in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::sync_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! sync_conformance {
    ($setup:expr) => {
        $crate::sync_conformance!(@cases $setup =>
            a_first_sync_commits_every_file_and_they_decode,
            a_mapped_prefix_decides_where_a_file_lands,
            a_top_level_mapping_takes_its_subtree_from_the_root_mapping,
            an_nfd_local_name_becomes_an_nfc_entry_path,
            a_walked_files_birth_time_reaches_the_record,
            an_unchanged_second_sync_commits_nothing,
            a_touched_file_with_equal_content_commits_nothing,
            a_modified_file_replaces_its_one_file_container,
            a_replacement_for_a_removed_container_is_refused,
            a_pack_resident_change_is_surfaced_and_untouched,
            a_file_deleted_locally_is_surfaced_and_untouched,
            an_entry_this_device_never_materialized_is_left_alone,
            an_unedited_file_this_device_deleted_goes_to_the_trash,
            an_unedited_file_another_device_deleted_goes_to_the_trash,
            an_edited_departed_file_is_kept_and_reported_every_run,
            a_touched_departed_file_with_its_recorded_content_is_unedited,
            a_departed_file_with_no_recorded_hash_is_kept_once_it_changed,
            a_path_a_new_entry_arrived_at_is_not_departed,
            a_file_put_where_one_was_moved_to_the_trash_is_new,
            a_departed_path_whose_file_is_gone_is_forgotten,
            a_departed_file_in_a_trash_folder_inside_the_root_is_not_carried_back,
            a_refused_move_to_the_trash_leaves_the_file_and_is_retried,
            nothing_is_moved_to_the_trash_through_a_refused_root,
            nothing_is_moved_to_the_trash_under_an_unavailable_root,
            a_freeze_leaves_a_departed_file_to_the_sync,
            a_missing_mapped_root_is_reported_and_infers_no_deletion,
            an_empty_root_on_another_filesystem_is_reported_and_infers_no_deletion,
            an_emptied_folder_on_the_recorded_filesystem_still_reports_its_deletions,
            a_renumbered_root_that_holds_files_is_restamped_and_scans_normally,
            an_unavailable_top_level_mapping_holds_its_subtree_back_from_the_root_mapping,
            a_mapping_recorded_afresh_clears_its_identity_and_reports_the_deletions,
            a_row_precedes_the_first_byte_of_a_spool,
            an_unfinished_spool_is_disposed_with_its_row,
            a_spooling_row_whose_spool_was_never_created_is_disposed,
            a_spool_left_by_an_interrupted_run_converges_to_one_entry,
            an_uploaded_but_uncommitted_container_converges_to_one_entry,
            an_uploaded_container_is_settled_by_the_next_run,
            an_uncertain_commit_retains_its_ciphertext_and_provenance,
            another_writers_head_does_not_prove_an_uncertain_attempt,
            a_live_producer_keeps_exclusive_ownership,
            a_lost_response_and_withheld_head_never_authorize_trash,
            an_abandoned_container_storage_will_not_trash_is_left_in_storage,
            a_stale_pending_row_is_dropped_with_its_spool,
            a_commit_whose_refresh_failed_is_completed_and_replaced,
            a_completed_container_marks_its_file_present,
            a_run_with_no_pending_rows_reads_the_head_once,
            a_device_that_maps_nothing_is_told_apart_from_a_folder_with_nothing_new,
            a_run_says_which_phase_it_is_in_and_counts_the_ones_it_can,
            sync_catches_up_before_scanning,
            a_provider_hash_mismatch_is_refused,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::sync_conformance::$case(&fixture).await,
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
