use crate::index_conformance::device_state::STAMPED;
use crate::index_conformance::fixtures::{mapping, stamped};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// Mappings are kept one per prefix, the Library root first (spec: EP-9).
pub async fn a_mapping_is_kept_once_per_prefix(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .set_mapping(mapping(Some("albums"), "/photos"))
        .await
        .expect("recording a mapping must succeed");
    index
        .set_mapping(mapping(None, "/data/library"))
        .await
        .expect("recording a root mapping must succeed");
    index
        .set_mapping(mapping(Some("albums"), "/photos-on-the-other-disk"))
        .await
        .expect("moving a mapping must succeed");

    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [
            mapping(None, "/data/library"),
            mapping(Some("albums"), "/photos-on-the-other-disk"),
        ]
    );
}

/// A mapping's recorded filesystem identity round-trips, and recording the
/// mapping afresh clears it (spec: EP-12).
///
/// The clearing is the half that matters, and it is not a convenience: it is the
/// gesture a device is left with when a folder it genuinely emptied stands on a
/// filesystem whose identity also moved. Such a root reports unavailable and
/// keeps doing so, because an empty root is never re-stamped — so recording the
/// mapping again with no identity is how the device says "this root is what I
/// meant", and the next scan stamps what is there and infers the deletions. A
/// catalog that kept the old identity through that call would leave the folder
/// stuck reporting unavailable forever.
pub async fn a_mapping_round_trips_its_root_identity(fixture: &IndexUnderTest) {
    /// What a scan writes over [`STAMPED`] when the disk comes back renumbered.
    const RESTAMPED: &str = "the-filesystem-a-later-scan-saw";

    let index = fixture.index();

    index
        .set_mapping(stamped(Some("albums"), "/photos", STAMPED))
        .await
        .expect("recording a stamped mapping must succeed");
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [stamped(Some("albums"), "/photos", STAMPED)],
        "a mapping read back is the mapping that was written, its identity included",
    );

    // The disk came back renumbered, and a scan re-stamped what it saw.
    index
        .set_mapping(stamped(Some("albums"), "/photos", RESTAMPED))
        .await
        .expect("re-stamping a mapping must succeed");
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [stamped(Some("albums"), "/photos", RESTAMPED)],
        "the identity moves with the rest of the row",
    );

    // The re-confirmation gesture: the same mapping, recorded with no identity.
    index
        .set_mapping(mapping(Some("albums"), "/photos"))
        .await
        .expect("recording a mapping afresh must succeed");
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading mappings must succeed"),
        [mapping(Some("albums"), "/photos")],
        "recording a mapping afresh clears the identity rather than keeping it",
    );
}
