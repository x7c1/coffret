//! The spool capability's contract, as tests.
//!
//! [`Spool`](crate::Spool) is where the ciphertext of a batch waits between
//! being encoded and being committed, and the flows above it keep promises about
//! what an interruption leaves behind (spec: OC-2, OC-6). Those promises are
//! only worth what the capability underneath them actually does, and there are
//! two implementations of it: the device's own disk, in the local filesystem
//! gateway, and the in-memory fake this crate's own cases run against. This
//! suite is what holds them to one contract, so that a case which passes in
//! memory says something about the disk.
//!
//! Each case takes a [`SpoolUnderTest`] — one spool and one directory, handed
//! over *unprepared* — and [`spool_conformance!`](crate::spool_conformance!)
//! turns the whole set into ordinary `#[tokio::test]` functions in a backend's
//! test target.
//!
//! It is behind the `conformance` feature, so that only test targets pay for it.

/// What one Container's ciphertext stands in for here.
///
/// Shared by the cases that write one, so that a spool read back is compared
/// against the same bytes wherever the case that wrote it lives.
const CIPHERTEXT: &[u8] = b"what a Container weighs, in a case that is not about that";

mod preparation;
pub use preparation::{
    a_directory_an_earlier_run_prepared_is_prepared_again,
    a_spool_under_an_unprepared_directory_is_refused,
};

mod removal;
pub use removal::a_discard_of_a_spool_that_is_already_gone_succeeds;

mod round_trip;
pub use round_trip::{
    a_created_spool_replaces_what_was_at_the_path, a_spool_is_written_flushed_read_back_and_removed,
};

mod spool_under_test;
pub use spool_under_test::SpoolUnderTest;

/// Declares the whole spool conformance suite as tests of the calling crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`SpoolUnderTest`]`>`: `Some` with a spool and a
/// directory the fixture has *not* made to run the case against, or `None` to
/// skip it because this implementation is not available in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::spool_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! spool_conformance {
    ($setup:expr) => {
        $crate::spool_conformance!(@cases $setup =>
            a_spool_is_written_flushed_read_back_and_removed,
            a_discard_of_a_spool_that_is_already_gone_succeeds,
            a_spool_under_an_unprepared_directory_is_refused,
            a_directory_an_earlier_run_prepared_is_prepared_again,
            a_created_spool_replaces_what_was_at_the_path,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::spool_conformance::$case(&fixture).await,
                    None => eprintln!(
                        concat!(
                            "skipping ",
                            stringify!($case),
                            ": no spool is available in this environment",
                        ),
                    ),
                }
            }
        )+
    };
}
