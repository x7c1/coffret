//! The [`ObjectStore`](crate::ObjectStore) contract, as tests.
//!
//! The port is only worth having if every adapter behind it behaves the same,
//! and the way to keep two adapters honest about that is one suite both of them
//! run rather than two suites written to what each provider happens to do. Each
//! case here takes a [`StoreUnderTest`] and asserts one piece of the contract;
//! [`object_store_conformance!`](crate::object_store_conformance) turns the
//! whole set into ordinary `#[tokio::test]` functions in a gateway's test
//! target.
//!
//! The module lives in the domain crate, next to the trait it is the contract
//! for, and does no I/O of its own: it drives whatever store a gateway hands
//! it. It is behind the `conformance` feature so that only test targets pay for
//! it.
//!
//! A gateway whose store is not always reachable — a real cloud account behind
//! credentials — hands back `None` and the cases report themselves skipped
//! rather than failing a run that was never configured to reach it.

mod conditional_create;
pub use conditional_create::{
    put_if_absent_rejects_a_taken_slot, put_if_absent_settles_a_race_between_two_writers,
    put_if_absent_takes_a_free_slot,
};

mod listing;
pub use listing::{
    list_is_empty_on_a_fresh_store, list_reports_what_it_stored, list_walks_every_page_exactly_once,
};

mod listing_walk;
pub use listing_walk::ListingWalk;

mod removal;
pub use removal::{
    purge_is_idempotent, purge_removes_a_live_object, purge_removes_a_trashed_object,
    trash_hides_an_object_from_list,
};

mod store_under_test;
pub use store_under_test::StoreUnderTest;

mod transfer;
pub use transfer::{
    get_reads_a_byte_range, get_reports_a_missing_object, put_get_round_trips_a_zero_length_object,
    put_get_round_trips_content,
};

/// Declares the whole conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`StoreUnderTest`]`>`: `Some` with an empty store to
/// run the case against, or `None` to skip it because this store is not
/// configured in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among
/// its dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::object_store_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! object_store_conformance {
    ($setup:expr) => {
        $crate::object_store_conformance!(@cases $setup =>
            put_get_round_trips_content,
            put_get_round_trips_a_zero_length_object,
            get_reads_a_byte_range,
            get_reports_a_missing_object,
            put_if_absent_takes_a_free_slot,
            put_if_absent_rejects_a_taken_slot,
            put_if_absent_settles_a_race_between_two_writers,
            list_is_empty_on_a_fresh_store,
            list_reports_what_it_stored,
            list_walks_every_page_exactly_once,
            trash_hides_an_object_from_list,
            purge_removes_a_live_object,
            purge_removes_a_trashed_object,
            purge_is_idempotent,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::conformance::$case(&fixture).await,
                    None => eprintln!(
                        concat!(
                            "skipping ",
                            stringify!($case),
                            ": no store is configured in this environment",
                        ),
                    ),
                }
            }
        )+
    };
}
