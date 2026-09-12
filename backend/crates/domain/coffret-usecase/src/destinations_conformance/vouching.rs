use coffret_model::Mtime;

use crate::descent_error::DescentError;
use crate::destinations_conformance::destinations_under_test::DestinationsUnderTest;
use crate::destinations_conformance::{components, registered, STAMPED};
use crate::device_state::RootMarkerId;
use crate::local_operation::LocalOperation;
use crate::refused_root::RootRefused;
use crate::root_marker;

/// The components every case here reaches for, which no case gets as far as
/// descending.
fn place() -> Vec<String> {
    components(&["albums", "spring.jpg"])
}

/// What the capability refused a reach with, or a panic naming what it answered
/// instead.
///
/// Only the reason: the root is the one the case handed over, so asserting on it
/// would assert about the fixture rather than about the capability.
async fn refusal(fixture: &DestinationsUnderTest, expected: Option<&RootMarkerId>) -> RootRefused {
    let refused = fixture
        .destinations()
        .reach(fixture.root(), expected, &place())
        .await
        .err()
        .expect("nothing may be placed into a root that will not vouch for itself");
    match refused {
        DescentError::Refused { reason, .. } => reason,
        other => panic!("a root that is not the registered one is refused, and it said {other:?}"),
    }
}

/// A mapped root holding no management area is refused, and nothing below it is
/// made.
///
/// The ordinary shape of the rule's whole point: a folder some registration never
/// visited says nothing about which folder it is, so a device that placed a file
/// into it would be putting the Library's content somewhere nobody pointed it
/// (spec: EP-13). Nothing is created on the way to finding that out — not the
/// folders an Entry Path's separators call for, and not the management area
/// either, since only recording a mapping ever writes one.
pub async fn a_root_with_no_management_area_refuses_the_reach(fixture: &DestinationsUnderTest) {
    let expected = RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN]);

    let reason = refusal(fixture, Some(&expected)).await;
    assert!(
        matches!(reason, RootRefused::ManagementAreaMissing),
        "a root with no {} folder holds no identity to compare: {reason:?}",
        root_marker::MANAGEMENT_AREA,
    );

    assert!(
        !fixture
            .arrange()
            .holds(&fixture.root().join(root_marker::MANAGEMENT_AREA)),
        "a placement neither creates nor repairs a management area (spec: EP-13)",
    );
    assert!(
        !fixture.arrange().holds(&fixture.root().join("albums")),
        "and it makes none of the folders below a root it would not write into",
    );
}

/// A management area holding no marker is refused as that, and told apart from a
/// root with no management area at all.
///
/// The interrupted registration EP-13 names by itself: recording a mapping makes
/// `.coffret` and then writes `root` inside it, so a run killed between the two
/// leaves exactly this. Its own case because the two shapes send a person to
/// different places — a root nobody registered, against a registration that did
/// not finish — and because a backend that answered either with the other's
/// verdict would say the wrong one of the two.
///
/// Arranged by putting one name of coffret's own inside the area, since making
/// the folder is the only way to have it: the marker's own name is deliberately
/// untouched, which is the whole of what the case is about.
pub async fn a_management_area_with_no_marker_refuses_the_reach(fixture: &DestinationsUnderTest) {
    let expected = RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN]);
    let area = fixture.root().join(root_marker::MANAGEMENT_AREA);
    fixture.arrange().write_file(
        &area.join("not-the-marker"),
        b"something of the device's own that is not the marker",
        Mtime::from_unix_seconds(STAMPED),
    );

    let reason = refusal(fixture, Some(&expected)).await;
    assert!(
        matches!(reason, RootRefused::MarkerMissing),
        "an area with no {} in it is an unfinished registration and not a missing area: \
         {reason:?}",
        root_marker::MARKER_FILE,
    );

    assert!(
        !fixture
            .arrange()
            .holds(&area.join(root_marker::MARKER_FILE)),
        "and the marker the registration never wrote is not written now (spec: EP-13)",
    );
    assert!(
        !fixture.arrange().holds(&fixture.root().join("albums")),
        "nor is any folder below a root that will not vouch for itself",
    );
}

/// A marker naming another identity is refused as a mismatch.
///
/// The case the rule exists for: the folder in front of the device is a folder
/// some coffret registered, and not the one this mapping was recorded against — a
/// disk mounted where another one used to be, or a copy of a registered folder.
/// Told apart from a root carrying no marker at all, because the two send a person
/// to different places (spec: EP-13).
pub async fn a_marker_naming_another_identity_refuses_the_reach(fixture: &DestinationsUnderTest) {
    let planted = registered(fixture);
    let expected = RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN]);
    assert_ne!(planted, expected, "a fixture's two identities differ");

    let reason = refusal(fixture, Some(&expected)).await;
    assert!(
        matches!(reason, RootRefused::MarkerMismatch),
        "a marker naming another identity is a mismatch and not a missing marker: {reason:?}",
    );

    assert_eq!(
        fixture
            .arrange()
            .content(
                &fixture
                    .root()
                    .join(root_marker::MANAGEMENT_AREA)
                    .join(root_marker::MARKER_FILE)
            )
            .as_deref(),
        Some(root_marker::spell(&planted).as_slice()),
        "and the marker is left exactly as it was: only recording a mapping writes one",
    );
}

/// Something other than a folder at the management area's name is refused as
/// that, marker or no.
///
/// `.coffret` is reserved for the device's own folder at any depth under a mapped
/// root (spec: EP-14), so a symbolic link or a file standing at the name is not
/// a folder to descend and not a name to follow — what a descent finds past such
/// a link is not this root's management area. One case for both, because what a
/// person has to do about either is the same.
pub async fn a_management_area_that_is_not_a_folder_refuses_the_reach(
    fixture: &DestinationsUnderTest,
) {
    let expected = RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN]);
    fixture
        .arrange()
        .plant_other(&fixture.root().join(root_marker::MANAGEMENT_AREA));

    let reason = refusal(fixture, Some(&expected)).await;
    assert!(
        matches!(reason, RootRefused::ManagementAreaNotADirectory),
        "the name is reserved and something else is standing at it: {reason:?}",
    );
}

/// A marker whose content names no identity is refused as malformed, with the
/// defect carried.
///
/// A marker is coffret's own file, so a file that is only nearly one names no
/// identity at all and is refused rather than read generously (spec: EP-13). The
/// content itself stays in the value and reaches no diagnostic event.
pub async fn a_marker_that_names_no_identity_refuses_the_reach(fixture: &DestinationsUnderTest) {
    let expected = RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN]);
    fixture.arrange().write_file(
        &fixture
            .root()
            .join(root_marker::MANAGEMENT_AREA)
            .join(root_marker::MARKER_FILE),
        b"not an identity at all",
        Mtime::from_unix_seconds(STAMPED),
    );

    let reason = refusal(fixture, Some(&expected)).await;
    assert!(
        matches!(reason, RootRefused::MarkerMalformed { .. }),
        "content that spells no identity is malformed: {reason:?}",
    );
}

/// A mapping recording no identity at all refuses, sound marker or not.
///
/// `None` is not a mapping that skips the check: there is nothing for the marker
/// to agree with, so there is no folder this device may vouch for (spec: EP-13).
/// A mapping read back out of a device-state file that predates the marker
/// arrives this way, and the root it names may be in perfect order — which is why
/// the case plants a valid marker and still expects the refusal.
pub async fn a_reach_with_no_expected_identity_refuses(fixture: &DestinationsUnderTest) {
    registered(fixture);

    let reason = refusal(fixture, None).await;
    assert!(
        matches!(reason, RootRefused::NoExpectedIdentity),
        "a mapping with no identity has nothing to compare, whatever the root holds: {reason:?}",
    );
}

/// The identity a case that never reaches the marker hands over.
///
/// Any value at all: what these two cases are about is that the root's own
/// questions are answered before the marker's is asked, so the reach is given a
/// sound-looking identity and still never gets as far as comparing one.
fn unreached() -> RootMarkerId {
    RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN])
}

/// A mapped root that is not there at all is refused as that, and never as
/// anything about the marker.
///
/// Where EP-12 and EP-13 meet, and the one place they must not be confused: a
/// root nothing is at has no marker question to answer, so a capability that
/// answered "no management area" for an unplugged disk would send a person to
/// record the mapping again over a disk they only have to plug in. It is an I/O
/// answer rather than a refusal because the root is *asked for* and never made
/// here — nothing creates a root nobody registered — so what comes back is what
/// the operating system says about a path nothing is at, reported against the
/// root and as a reading of it (spec: EP-12, EP-13).
pub async fn a_missing_root_says_nothing_about_the_marker(fixture: &DestinationsUnderTest) {
    // Under the case's own root, so that nothing outside the fixture is named.
    // What makes it a missing root is that nothing arranged it.
    let root = fixture.root().join("a-disk-that-is-not-plugged-in");

    let refused = fixture
        .destinations()
        .reach(&root, Some(&unreached()), &place())
        .await
        .err()
        .expect("there is nowhere to place a file and nothing to ask about a marker");

    match refused {
        DescentError::Io(refused) => {
            assert_eq!(refused.path, root, "the root is what the refusal is about");
            assert!(
                matches!(refused.operation, LocalOperation::Stating),
                "the root is opened and never made, so a refusal here may not tell a person \
                 coffret failed to create their folder: {:?}",
                refused.operation,
            );
        }
        other => {
            panic!("a root that is not there is an I/O answer, and the capability said {other:?}")
        }
    }

    assert!(
        !fixture.arrange().holds(&root),
        "and the root no registration ever visited is not made on the way past (spec: EP-13)",
    );
}

/// Something other than a folder at the mapped root's own name blocks the reach,
/// and the root is the name the refusal carries.
///
/// Nothing makes the root any more, which puts it among the names a descent can
/// meet the wrong kind of thing at (spec: EP-13). The verdict is the one EP-4
/// already has for such a name rather than anything about the marker: what
/// stands there is not a folder the management area could be descended from, and
/// no file on this device can stand for the Entry Path while it does
/// (spec: EP-4, EP-11).
pub async fn a_root_that_is_not_a_folder_blocks_the_reach(fixture: &DestinationsUnderTest) {
    // A file where a mapping's root points, which is what typing one path for
    // another leaves behind.
    let root = fixture.root().join("a-file-the-mapping-points-at");
    fixture.arrange().write_file(
        &root,
        b"a file of the person's own, where a mapped root would go",
        Mtime::from_unix_seconds(STAMPED),
    );

    let refused = fixture
        .destinations()
        .reach(&root, Some(&unreached()), &place())
        .await
        .err()
        .expect("a mapped root that is not a folder is not one to place under");

    assert!(
        matches!(refused, DescentError::Blocked { ref path } if path == &root),
        "the root is the name there is to look at, nothing below it having been reached: \
         {refused:?}",
    );
}
