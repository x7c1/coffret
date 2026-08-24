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
//! One case needs Storage to misreport what it stored, and reaches it by
//! wrapping whatever store the backend handed over, which keeps it
//! backend-agnostic: the same disagreement runs against a real provider and in
//! memory.
//!
//! The module lives in the domain crate, next to the flow it is the contract
//! for. It reads and writes files, which the other three suites do not — a sync
//! starts at a folder — but only under the two directories the backend hands
//! it. It is behind the `conformance` feature so that only test targets pay for
//! it.

mod fixtures;

mod import;
pub use import::{
    a_first_sync_commits_every_file_and_they_decode, a_mapped_prefix_decides_where_a_file_lands,
    a_top_level_mapping_takes_its_subtree_from_the_root_mapping,
};

mod integrity;
pub use integrity::a_provider_hash_mismatch_is_refused;

mod interruption;
pub use interruption::{
    a_spool_left_by_an_interrupted_run_converges_to_one_entry,
    a_stale_pending_row_is_dropped_with_its_spool,
    an_uploaded_but_uncommitted_container_converges_to_one_entry,
    an_uploaded_container_waits_for_a_run_that_reads_the_head,
};

mod library;

mod mangling_store;

mod modification;
pub use modification::{
    a_modified_file_replaces_its_one_file_container,
    a_pack_resident_change_is_surfaced_and_untouched,
};

mod repeat;
pub use repeat::{
    a_touched_file_with_equal_content_commits_nothing, an_unchanged_second_sync_commits_nothing,
};

mod scope;
pub use scope::{
    a_file_deleted_locally_is_surfaced_and_untouched,
    an_entry_this_device_never_materialized_is_left_alone,
};

mod sync_under_test;
pub use sync_under_test::SyncUnderTest;

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
            an_unchanged_second_sync_commits_nothing,
            a_touched_file_with_equal_content_commits_nothing,
            a_modified_file_replaces_its_one_file_container,
            a_pack_resident_change_is_surfaced_and_untouched,
            a_file_deleted_locally_is_surfaced_and_untouched,
            an_entry_this_device_never_materialized_is_left_alone,
            a_spool_left_by_an_interrupted_run_converges_to_one_entry,
            an_uploaded_but_uncommitted_container_converges_to_one_entry,
            an_uploaded_container_waits_for_a_run_that_reads_the_head,
            a_stale_pending_row_is_dropped_with_its_spool,
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
