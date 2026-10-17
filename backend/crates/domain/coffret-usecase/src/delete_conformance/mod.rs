//! The deletion's contract, as tests.
//!
//! What a deletion can get wrong is not what packing or fetching can. It can
//! remove a Container that still holds an Entry nobody named; it can write a
//! replacement that drops a kept Entry, reorders them, or records a time for a
//! file it never saw; it can write one out of bytes it did not verify; it can
//! take part of a Pack nothing can read and invent the rest; and it can tell a
//! person one thing in a preview and do another. Each case here is one of
//! those, held to the rule it would break (spec: PK-9, PK-10, PK-15, KL-17,
//! CP-1, CP-14).
//!
//! What a case asserts is read off Storage the way another device would read
//! it: the replacement is opened under the envelope the committed Keyring maps
//! it to and decoded, and what is trashed is what the listing no longer holds.
//! The Library is put together by the flows that put one together — a sync for
//! one-file Containers, a freeze for Packs — so a case starts from the state a
//! person's Library is actually in. Three states no flow produces are borrowed
//! from the suites that already arrange them: a committed key-lost marker
//! (spec: KL-7), a Container damaged in transit, and a provider that will not
//! trash anything — and a fourth, a catalog that refuses the refresh after a
//! commit has landed, which is where a rebuilt Pack's own bookkeeping is put
//! to the test (spec: OC-7).
//!
//! And it can leave the deleting device's own copies of what it deleted
//! standing in the folder as if the deletion had not happened, or take one the
//! person edited since; those cases hold it to the decision a sync makes about
//! the same files, under the same root checks (spec: EP-12, EP-13, EP-15).
//!
//! It is behind the `conformance` feature so that only test targets pay for it.

mod completion;
pub use completion::a_rebuild_whose_refresh_failed_is_completed_without_claiming_its_files;

mod delete_under_test;
pub use delete_under_test::DeleteUnderTest;

mod fixtures;

mod local_copies;
pub use local_copies::{
    a_copy_gone_from_disk_is_left_alone,
    a_deletion_on_another_device_leaves_this_devices_copies_to_its_sync,
    a_refused_move_leaves_the_deletion_committed_and_is_reported,
    a_root_that_does_not_vouch_for_itself_leaves_the_copy_to_the_next_sync,
    an_edited_copy_is_kept_and_reported, an_unavailable_root_is_reported_and_its_copies_left_alone,
    the_deleting_devices_unedited_copies_go_to_the_trash_after_the_commit,
};

mod preview;
pub use preview::{
    a_preview_counts_what_the_deletion_then_does,
    the_committed_keyring_names_the_packs_a_preview_refuses,
};

mod rebuild;
pub use rebuild::{
    a_folder_spanning_several_packs_is_deleted_in_one_batch,
    a_pack_that_keeps_entries_is_rebuilt_with_exactly_them,
    a_pack_whose_kept_entry_was_renamed_is_rebuilt_under_its_current_path,
};

mod refusal;
pub use refusal::{
    a_pack_that_does_not_verify_is_refused_and_the_rest_commits,
    a_partial_deletion_of_a_key_lost_pack_is_refused,
};

mod stale_removal;
pub use stale_removal::a_rebuild_of_a_pack_another_device_removed_is_refused;

mod removal;
pub use removal::{
    a_pack_whose_entries_are_all_deleted_is_removed_even_with_its_key_lost,
    a_removal_that_will_not_go_to_the_trash_leaves_the_deletion_committed,
    an_entry_in_a_one_file_container_is_deleted_by_removing_it,
};

/// Creates the Library a case runs in, in the empty store the backend handed
/// over: its first head, written as creating a Library on a device writes it
/// (spec: CP-2, FM-13). Public only for the macro, which calls it before each
/// case.
#[doc(hidden)]
pub async fn create_library(fixture: &DeleteUnderTest) {
    let keys = fixtures::keys();
    crate::commit_conformance::create_library_in(
        fixture.store(),
        keys.control(),
        &fixtures::policy(),
    )
    .await;
}

/// Declares the whole deletion conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`DeleteUnderTest`]`>`: `Some` with an empty store
/// and two empty catalogs to run the case against, or `None` to skip it because
/// this backend is not configured in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::delete_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! delete_conformance {
    ($setup:expr) => {
        $crate::delete_conformance!(@cases $setup =>
            an_entry_in_a_one_file_container_is_deleted_by_removing_it,
            a_pack_whose_entries_are_all_deleted_is_removed_even_with_its_key_lost,
            a_removal_that_will_not_go_to_the_trash_leaves_the_deletion_committed,
            a_pack_that_keeps_entries_is_rebuilt_with_exactly_them,
            a_pack_whose_kept_entry_was_renamed_is_rebuilt_under_its_current_path,
            a_rebuild_of_a_pack_another_device_removed_is_refused,
            a_folder_spanning_several_packs_is_deleted_in_one_batch,
            a_pack_that_does_not_verify_is_refused_and_the_rest_commits,
            a_partial_deletion_of_a_key_lost_pack_is_refused,
            a_rebuild_whose_refresh_failed_is_completed_without_claiming_its_files,
            a_preview_counts_what_the_deletion_then_does,
            the_committed_keyring_names_the_packs_a_preview_refuses,
            the_deleting_devices_unedited_copies_go_to_the_trash_after_the_commit,
            an_edited_copy_is_kept_and_reported,
            a_copy_gone_from_disk_is_left_alone,
            a_root_that_does_not_vouch_for_itself_leaves_the_copy_to_the_next_sync,
            an_unavailable_root_is_reported_and_its_copies_left_alone,
            a_refused_move_leaves_the_deletion_committed_and_is_reported,
            a_deletion_on_another_device_leaves_this_devices_copies_to_its_sync,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => {
                        $crate::delete_conformance::create_library(&fixture).await;
                        $crate::delete_conformance::$case(&fixture).await
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
