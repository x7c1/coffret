//! The freeze's contract, as tests.
//!
//! [`sync_conformance`](mod@crate::sync_conformance) holds a store, a catalog,
//! and a folder to what carrying that folder into the Library one Container per
//! file means. This is the suite over the operation that groups those files
//! instead, and it is a suite of its own because what it can get wrong is not
//! what a sync can. A sync that is idempotent, verified, and recoverable still
//! leaves open whether the Packs come out in path order, whether the target is
//! respected and respected as late as it could be, whether an indivisible Entry
//! larger than the target is split, whether a second run quietly rewrites what
//! the first built, and whether a folder held in Packs can be read back at all.
//!
//! Every case runs a **tiny size target**, which is the whole trick that keeps
//! the suite cheap: the boundary the segmentation rule is about is crossed
//! several times over by a handful of short files, so nothing here has to write
//! a gigabyte to reach the behavior a gigabyte would (spec: PK-5).
//!
//! What a case asserts is deliberately not what the run returned. The Packs are
//! fetched back off Storage, opened through the envelope the committed Keyring
//! maps them to under a purpose key derived from the Master Key, and decoded —
//! because packing is worth exactly what another enrolled device can open
//! afterwards. The round-trip case goes further and drives a real
//! [`fetch`](crate::fetch) from a **second device** with an empty catalog, so
//! the claim is about files on that device's disk rather than about bytes in a
//! bucket.
//!
//! Three things have to be arranged that no flow produces. A store that records
//! which objects a run touched, because "an existing Pack was left alone" is a
//! claim about an absence that nothing the flow returns can carry. A committed
//! Keyring recording a key as lost, which is written by hand for the reason a
//! commit refuses to invent one — losing a key is not something a commit does
//! (spec: KL-7). And a catalog that watches every spool announcement against the
//! disk and can refuse to record one as complete, which is how the ordering
//! inside a spool step is checked and how a run is stopped where it leaves a
//! Pack spool behind that no row calls whole. The last two are borrowed — from
//! the fetch suite and from the sync suite — rather than written a second time.
//!
//! The module lives in the domain crate, next to the flow it is the contract
//! for. It reads and writes files, as the sync and fetch suites do — a freeze
//! starts at a folder — but only under the directories the backend hands it. It
//! is behind the `conformance` feature so that only test targets pay for it.

mod absorption;
pub use absorption::{
    a_repeated_freeze_selects_nothing_and_leaves_packs_untouched,
    previously_synced_containers_are_absorbed,
};

mod counting_store;

mod fixtures;

mod freeze_under_test;
pub use freeze_under_test::FreezeUnderTest;

mod import;
pub use import::{
    a_file_larger_than_the_target_forms_a_singleton_pack, a_folder_freezes_into_path_ordered_packs,
    a_prefix_narrows_the_run_to_one_folder,
};

mod interruption;
pub use interruption::{
    a_provisional_pack_row_is_never_uploaded_or_committed,
    a_row_precedes_the_first_byte_of_a_pack_spool,
    an_unfinished_pack_spool_is_disposed_with_its_row,
};

mod recovery;
pub use recovery::{
    a_key_lost_one_file_entry_freezes_to_the_local_bytes,
    a_modified_one_file_entry_freezes_to_the_local_bytes,
};

mod round_trip;
pub use round_trip::a_second_device_fetches_a_frozen_folder;

mod surfacing;
pub use surfacing::{
    a_key_lost_pack_entry_is_surfaced_and_untouched,
    a_modified_pack_resident_entry_is_surfaced_and_untouched,
    a_touched_pack_resident_entry_is_not_a_finding,
};

/// Declares the whole freeze conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`FreezeUnderTest`]`>`: `Some` with an empty store,
/// two empty catalogs, and three empty directories to run the case against, or
/// `None` to skip it because this backend is not configured in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::freeze_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! freeze_conformance {
    ($setup:expr) => {
        $crate::freeze_conformance!(@cases $setup =>
            a_folder_freezes_into_path_ordered_packs,
            a_file_larger_than_the_target_forms_a_singleton_pack,
            a_prefix_narrows_the_run_to_one_folder,
            previously_synced_containers_are_absorbed,
            a_repeated_freeze_selects_nothing_and_leaves_packs_untouched,
            a_modified_one_file_entry_freezes_to_the_local_bytes,
            a_key_lost_one_file_entry_freezes_to_the_local_bytes,
            a_modified_pack_resident_entry_is_surfaced_and_untouched,
            a_touched_pack_resident_entry_is_not_a_finding,
            a_key_lost_pack_entry_is_surfaced_and_untouched,
            a_row_precedes_the_first_byte_of_a_pack_spool,
            an_unfinished_pack_spool_is_disposed_with_its_row,
            a_provisional_pack_row_is_never_uploaded_or_committed,
            a_second_device_fetches_a_frozen_folder,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::freeze_conformance::$case(&fixture).await,
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
