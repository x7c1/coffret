//! The fetch's contract, as tests.
//!
//! [`sync_conformance`](mod@crate::sync_conformance) holds a store, a catalog,
//! and a folder to what carrying that folder into the Library means. This is the
//! suite over the journey back, and it is a suite of its own because what it can
//! get wrong is not what a sync can. A sync that is idempotent, verified, and
//! recoverable still leaves open whether a *second* device can rebuild the
//! catalog from Storage alone, whether the file that lands is the file that left,
//! whether a fetch overwrites something the user never synced, and whether a
//! second fetch of an untouched folder pulls every Container down again.
//!
//! Every case runs **two devices** against one store, which is the whole shape of
//! the suite. One syncs a folder and the other fetches it, and the two catalogs
//! share nothing — no mappings, no materialization records, no checkpoint — so the
//! fetching device's catch-up is a real restore-and-replay rather than a no-op
//! (spec: CK-9, RV-1). Nothing a run returns is taken as evidence about the
//! files: the cases read what is on the target device's disk, because a fetch is
//! worth exactly what is in the folder afterwards.
//!
//! Three things have to be arranged that no flow produces. A store that damages
//! one object between the bucket and the device, because bytes lost in transit
//! cannot be reached by driving the flow and writing damage into the bucket would
//! ask a different question. A store that counts what a run reads, because "this
//! Entry was skipped" and "this Container was not fetched" are different claims.
//! And a committed Keyring that records a key as lost, which is written by hand
//! for the reason a commit refuses to invent one: losing a key is not something a
//! commit does (spec: KL-7). The two stores wrap whatever the backend handed the
//! suite, so the same fault and the same count happen against a real provider and
//! in memory.
//!
//! The module lives in the domain crate, next to the flow it is the contract for.
//! It reads and writes files, as the sync suite does — a fetch ends at a folder —
//! but only under the directories the backend hands it. It is behind the
//! `conformance` feature so that only test targets pay for it.

mod conflicts;
pub use conflicts::{
    a_foreign_file_is_surfaced_and_left_untouched,
    a_locally_changed_file_is_surfaced_and_left_untouched,
    a_witnessed_deletion_is_surfaced_and_not_refetched,
};

mod counting_store;

mod fetch_under_test;
pub use fetch_under_test::FetchUnderTest;

mod fixtures;

mod integrity;
pub use integrity::{
    a_container_that_does_not_decode_is_refused, a_container_whose_ciphertext_differs_is_refused,
    a_container_whose_content_is_not_what_the_catalog_names_is_refused,
};

mod keyring;
pub use keyring::{
    a_key_lost_container_is_locked_and_the_rest_is_fetched,
    a_mangled_first_keyring_replica_falls_back,
};

mod mangling_store;

mod round_trip;
pub use round_trip::{
    a_repeated_fetch_skips_everything_and_reads_no_container,
    a_second_device_fetches_a_synced_folder,
};

mod scope;
pub use scope::{
    a_mapped_prefix_decides_where_a_fetched_file_lands, a_prefix_narrows_the_fetch_to_one_subtree,
};

/// Declares the whole fetch conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`FetchUnderTest`]`>`: `Some` with an empty store, two
/// empty catalogs, and three empty directories to run the case against, or `None`
/// to skip it because this backend is not configured in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::fetch_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! fetch_conformance {
    ($setup:expr) => {
        $crate::fetch_conformance!(@cases $setup =>
            a_second_device_fetches_a_synced_folder,
            a_repeated_fetch_skips_everything_and_reads_no_container,
            a_prefix_narrows_the_fetch_to_one_subtree,
            a_mapped_prefix_decides_where_a_fetched_file_lands,
            a_foreign_file_is_surfaced_and_left_untouched,
            a_locally_changed_file_is_surfaced_and_left_untouched,
            a_witnessed_deletion_is_surfaced_and_not_refetched,
            a_container_that_does_not_decode_is_refused,
            a_container_whose_ciphertext_differs_is_refused,
            a_container_whose_content_is_not_what_the_catalog_names_is_refused,
            a_key_lost_container_is_locked_and_the_rest_is_fetched,
            a_mangled_first_keyring_replica_falls_back,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::fetch_conformance::$case(&fixture).await,
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
