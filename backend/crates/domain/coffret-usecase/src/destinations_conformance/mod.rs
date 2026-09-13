//! The destinations capability's contract, as tests.
//!
//! [`Destinations`](crate::Destinations) is how a local writer puts a file into
//! a folder this device maps — a fetch is one such writer, an upload the browser
//! drops is the other — and the rules above it are the ones EP-4 and
//! EP-11 set: a folder is reached by walking the Entry Path's components and
//! never by resolving a joined path, a component that is not a real folder of
//! that root refuses the whole place rather than being followed, and a file
//! becomes visible in one step — written, flushed, stamped, renamed — or not at
//! all. Those readings are only worth what the capability underneath them
//! actually answers, and there are two implementations of it: the device's own
//! disk, in the local filesystem gateway, and the in-memory fake this crate's
//! own cases run against. This suite is what holds them to one contract, so that
//! a case which passes in memory says something about the disk.
//!
//! Each case takes a [`DestinationsUnderTest`] — the capability, a
//! [`RootArrangement`] that puts folders and files where it will find them, one
//! mapped root that already exists, and a way to read a file back — and
//! [`destinations_conformance!`](crate::destinations_conformance!) turns the
//! whole set into ordinary `#[tokio::test]` functions in a backend's test
//! target.
//!
//! It is behind the `conformance` feature, so that only test targets pay for it.

use coffret_model::Mtime;

use crate::device_state::RootMarkerId;
use crate::root_marker;

/// What a case's file holds, when what it holds is not the point.
///
/// Shared by the cases that write one, so that a file read back is compared
/// against the same bytes wherever the case that wrote it lives.
const CONTENT: &[u8] = b"what a placed Entry weighs, in a case that is not about that";

/// The modification time a case stamps a placed file with (spec: FM-9).
///
/// A value rather than the clock's, because what a case asserts is that *this*
/// time reached the file: a stamp taken from "now" would make the assertion
/// depend on when the case ran.
const STAMPED: i64 = 1_600_000_000;

/// The Entry Path's components below the mapping's prefix, as the capability
/// takes them.
fn components(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

/// The identity every case's mapped root is registered under (spec: EP-13).
///
/// A value rather than a drawn one, for the reason [`STAMPED`] is one: what a
/// case asserts is that *this* identity was the one compared, and identities
/// drawn per run would make the mismatch case depend on which two the run
/// happened to draw.
const REGISTERED: [u8; RootMarkerId::BYTE_LEN] = [0x2a; RootMarkerId::BYTE_LEN];

/// Gives the case's mapped root the marker a placement compares against, and
/// hands back the identity its mapping records (spec: EP-13).
///
/// Every case that reaches a place needs it, because reaching one is what makes
/// the comparison: a root no registration ever visited is a root nothing may be
/// placed into. Planted through the arrangement rather than through the
/// capability, for the reason the suite plants everything that way — a fixture
/// that wrote the marker with the thing under test would prove only that it
/// agrees with itself.
fn registered(fixture: &DestinationsUnderTest) -> RootMarkerId {
    let id = RootMarkerId::from_bytes(REGISTERED);
    fixture.arrange().write_file(
        &fixture
            .root()
            .join(root_marker::MANAGEMENT_AREA)
            .join(root_marker::MARKER_FILE),
        &root_marker::spell(&id),
        Mtime::from_unix_seconds(STAMPED),
    );
    id
}

mod blocking;
pub use blocking::{
    a_file_where_a_folder_must_be_blocks_the_reach,
    a_symlink_on_the_way_blocks_the_reach_and_names_it,
};

mod destinations_under_test;
pub use destinations_under_test::DestinationsUnderTest;

mod looking;
pub use looking::{
    a_look_at_a_file_reports_its_size_and_time, a_look_at_an_empty_place_finds_nothing,
};

mod removal;
pub use removal::a_removal_of_a_name_that_is_already_gone_succeeds;

mod root_arrangement;
pub use root_arrangement::RootArrangement;

mod round_trip;
pub use round_trip::{
    a_place_is_written_flushed_stamped_and_published, a_publish_replaces_what_stood_at_the_name,
    a_scratch_name_that_is_taken_is_refused,
};

mod vouching;
pub use vouching::{
    a_management_area_that_is_not_a_folder_refuses_the_reach,
    a_management_area_with_no_marker_refuses_the_reach,
    a_marker_naming_another_identity_refuses_the_reach,
    a_marker_that_names_no_identity_refuses_the_reach,
    a_missing_root_says_nothing_about_the_marker, a_reach_with_no_expected_identity_refuses,
    a_root_that_is_not_a_folder_blocks_the_reach, a_root_with_no_management_area_refuses_the_reach,
};

/// Declares the whole destinations conformance suite as tests of the calling
/// crate.
///
/// The argument is an expression, evaluated afresh inside each generated test,
/// that awaits an `Option<`[`DestinationsUnderTest`]`>`: `Some` with a
/// capability, an arrangement, and a mapped root that already exists to run the
/// case against, or `None` to skip it because this implementation is not
/// available in this environment.
///
/// The calling crate needs `tokio` with its `macros` and `rt` features among its
/// dev-dependencies, since the cases are async.
///
/// ```ignore
/// coffret_usecase::destinations_conformance!(my_fixture().await);
/// ```
#[macro_export]
macro_rules! destinations_conformance {
    ($setup:expr) => {
        $crate::destinations_conformance!(@cases $setup =>
            a_place_is_written_flushed_stamped_and_published,
            a_publish_replaces_what_stood_at_the_name,
            a_scratch_name_that_is_taken_is_refused,
            a_removal_of_a_name_that_is_already_gone_succeeds,
            a_symlink_on_the_way_blocks_the_reach_and_names_it,
            a_file_where_a_folder_must_be_blocks_the_reach,
            a_look_at_an_empty_place_finds_nothing,
            a_look_at_a_file_reports_its_size_and_time,
            a_root_with_no_management_area_refuses_the_reach,
            a_management_area_with_no_marker_refuses_the_reach,
            a_marker_naming_another_identity_refuses_the_reach,
            a_management_area_that_is_not_a_folder_refuses_the_reach,
            a_marker_that_names_no_identity_refuses_the_reach,
            a_reach_with_no_expected_identity_refuses,
            a_missing_root_says_nothing_about_the_marker,
            a_root_that_is_not_a_folder_blocks_the_reach,
        );
    };
    (@cases $setup:expr => $($case:ident),+ $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                match $setup {
                    Some(fixture) => $crate::destinations_conformance::$case(&fixture).await,
                    None => eprintln!(
                        concat!(
                            "skipping ",
                            stringify!($case),
                            ": no destinations capability is available in this environment",
                        ),
                    ),
                }
            }
        )+
    };
}
