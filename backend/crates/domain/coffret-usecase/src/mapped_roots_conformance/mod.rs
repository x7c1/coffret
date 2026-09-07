//! The mapped-roots capability's contract, as tests.
//!
//! [`MappedRoots`](crate::MappedRoots) is how a sync and a freeze see the
//! folders this device maps into the Library, and the verdicts above it are
//! verdicts about *absence*: a root that is not there says nothing about the
//! Library rather than saying every Entry under it is gone (spec: EP-12), a
//! subfolder that went away mid-walk holds no more files, and a symbolic link is
//! never the file it points at (spec: EP-8). Those readings are only worth what
//! the capability underneath them actually answers, and there are two
//! implementations of it: the device's own disk, in the local filesystem
//! gateway, and the in-memory fake this crate's own cases run against. This
//! suite is what holds them to one contract, so that a case which passes in
//! memory says something about the disk.
//!
//! Each case takes a [`MappedRootsUnderTest`] — the capability, a
//! [`FolderArrangement`] that puts folders and files where it will find them,
//! and one directory that already exists — and
//! [`mapped_roots_conformance!`](crate::mapped_roots_conformance!) turns the
//! whole set into ordinary `#[tokio::test]` functions in a backend's test
//! target.
//!
//! It is behind the `conformance` feature, so that only test targets pay for it.

mod folder_arrangement;
pub use folder_arrangement::FolderArrangement;

mod listing;
pub use listing::{
    a_listing_reports_a_files_size_and_mtime_and_a_folder_as_a_folder,
    a_missing_folder_lists_to_nothing, listing_something_that_is_not_a_folder_is_refused,
    something_that_is_neither_a_file_nor_a_folder_lists_as_other,
};

mod mapped_roots_under_test;
pub use mapped_roots_under_test::MappedRootsUnderTest;

mod probing;
pub use probing::{
    a_missing_root_probes_to_nothing, a_present_root_probes_to_an_identity,
    probing_a_root_that_is_a_regular_file_answers_and_leaves_the_refusal_to_the_listing,
};

mod reading;
pub use reading::{
    a_source_streams_back_the_bytes_that_were_written,
    opening_a_missing_source_is_refused_as_reading,
};

/// Declares the whole mapped-roots conformance suite as tests of the calling
/// crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`MappedRootsUnderTest`]`>`: `Some` with a capability,
/// an arrangement, and a directory that already exists to run the case against,
/// or `None` to skip it because this implementation is not available in this
/// environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::mapped_roots_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! mapped_roots_conformance {
    ($setup:expr) => {
        $crate::mapped_roots_conformance!(@cases $setup =>
            a_missing_root_probes_to_nothing,
            a_present_root_probes_to_an_identity,
            probing_a_root_that_is_a_regular_file_answers_and_leaves_the_refusal_to_the_listing,
            a_missing_folder_lists_to_nothing,
            listing_something_that_is_not_a_folder_is_refused,
            a_listing_reports_a_files_size_and_mtime_and_a_folder_as_a_folder,
            something_that_is_neither_a_file_nor_a_folder_lists_as_other,
            a_source_streams_back_the_bytes_that_were_written,
            opening_a_missing_source_is_refused_as_reading,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::mapped_roots_conformance::$case(&fixture).await,
                    None => eprintln!(
                        concat!(
                            "skipping ",
                            stringify!($case),
                            ": no mapped-roots capability is available in this environment",
                        ),
                    ),
                }
            }
        )+
    };
}
