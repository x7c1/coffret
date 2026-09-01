//! The folder sync's contract, as tests.
//!
//! [`commit_conformance`](mod@crate::commit_conformance) holds a store and a
//! catalog to what a commit over both of them means. This is the suite over the
//! step before it — the one that turns a folder on a device into the batch a
//! commit takes — and it is a suite of its own because what it can get wrong is
//! not what a commit can. A commit that is exclusive, replayable, and complete
//! still leaves open whether a second sync of an untouched folder uploads
//! everything again, whether a file that was merely touched costs an upload,
//! whether a run killed mid-batch leaves two Entries or none, and whether the
//! ciphertext that reached Storage is the file that was on disk.
//!
//! Each case takes a [`SyncUnderTest`] — one store, one catalog, a folder, and
//! a spool directory — and drives [`sync_folders`](crate::sync::sync_folders)
//! against it; [`sync_conformance!`](crate::sync_conformance!) turns the whole
//! set into ordinary `#[tokio::test]` functions in a backend's test target.
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
//! for. It reads and writes files, which the other three suites do not — a sync
//! starts at a folder — but only under the two directories the backend hands
//! it. It is behind the `conformance` feature so that only test targets pay for
//! it.

mod completion;
pub use completion::{
    a_commit_whose_refresh_failed_is_completed_and_replaced,
    a_completed_container_marks_its_file_present, a_run_with_no_pending_rows_reads_no_head,
};

mod counting_store;

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

mod interruption;
pub use interruption::{
    a_row_precedes_the_first_byte_of_a_spool,
    a_spool_left_by_an_interrupted_run_converges_to_one_entry,
    a_spooling_row_whose_spool_was_never_created_is_disposed,
    a_stale_pending_row_is_dropped_with_its_spool, an_unfinished_spool_is_disposed_with_its_row,
    an_uploaded_but_uncommitted_container_converges_to_one_entry,
    an_uploaded_container_is_settled_by_the_next_run,
};

mod mangling_store;

mod modification;
pub use modification::{
    a_modified_file_replaces_its_one_file_container,
    a_pack_resident_change_is_surfaced_and_untouched,
};

mod refusing_index;

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

mod sync_under_test;
pub use sync_under_test::SyncUnderTest;

// Visible to the freeze suite, which borrows it: the Pack spool step keeps the
// same ordering, and one account of what that means is enough.
pub(crate) mod watching_index;

/// Declares the whole sync conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`SyncUnderTest`]`>`: `Some` with an empty store, an
/// empty catalog, an empty folder, and an empty spool directory to run the case
/// against, or `None` to skip it because this backend is not configured in this
/// environment.
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
            a_pack_resident_change_is_surfaced_and_untouched,
            a_file_deleted_locally_is_surfaced_and_untouched,
            an_entry_this_device_never_materialized_is_left_alone,
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
            a_stale_pending_row_is_dropped_with_its_spool,
            a_commit_whose_refresh_failed_is_completed_and_replaced,
            a_completed_container_marks_its_file_present,
            a_run_with_no_pending_rows_reads_no_head,
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
