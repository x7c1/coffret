use coffret_model::Mtime;

use crate::descent_error::DescentError;
use crate::destinations_conformance::components;
use crate::destinations_conformance::destinations_under_test::DestinationsUnderTest;
use crate::destinations_conformance::{CONTENT, STAMPED};
use crate::local_operation::LocalOperation;

/// The name a case's temporary file goes by.
///
/// Coffret's own reserved scratch prefix, because that is what a placement uses
/// and what a scan steps over (spec: EP-8, EP-11) — a suite that made up a name
/// of its own would be exercising a folder no real run leaves behind.
const SCRATCH: &str = ".coffret-fetch-a-case.part";

/// The whole of a placement: the folders are made, the bytes are written and
/// flushed, the time is stamped, and the rename is what makes the file exist.
///
/// One case rather than five, because the steps are only meaningful in order.
/// What EP-11 promises is that the file at the final name is whole, stamped, and
/// nowhere to be seen until the rename — so the assertions are about the state
/// *after* the rename and about the scratch name being gone, which is what says
/// the rename moved the file rather than copying it.
///
/// The folders on the way are the other half. An Entry Path's separators are the
/// whole of what a folder is (spec: EP-2), so a device placing
/// `albums/2026/spring.jpg` into an empty mapped root has to make both — and the
/// file has to land at exactly the path the components spell.
pub async fn a_place_is_written_flushed_stamped_and_published(fixture: &DestinationsUnderTest) {
    let components = components(&["albums", "2026", "spring.jpg"]);

    let destination = fixture
        .destinations()
        .reach(fixture.root(), &components)
        .await
        .expect("reaching a place under a root of real folders must succeed");
    let mut scratch = destination
        .create(SCRATCH)
        .expect("creating a temporary file in the folder must succeed");
    scratch
        .write(CONTENT)
        .await
        .expect("writing the plaintext must succeed");
    let mut flushed = scratch
        .flush()
        .await
        .expect("flushing it to the device must succeed");
    flushed
        .stamp(Mtime::from_unix_seconds(STAMPED))
        .await
        .expect("stamping the Entry's own time must succeed");
    flushed.publish().expect("the rename must succeed");

    let placed = fixture
        .root()
        .join("albums")
        .join("2026")
        .join("spring.jpg");
    assert_eq!(
        fixture.arrange().content(&placed).as_deref(),
        Some(CONTENT),
        "the file stands at the path the components spell, whole",
    );
    assert_eq!(
        fixture.arrange().mtime(&placed),
        Some(Mtime::from_unix_seconds(STAMPED)),
        "carrying the time its Entry records rather than the clock's (spec: FM-9)",
    );
    assert!(
        !fixture
            .arrange()
            .holds(&fixture.root().join("albums").join("2026").join(SCRATCH)),
        "the rename moved the temporary file rather than leaving a copy of it",
    );
}

/// Publishing over a file that already stands at the name replaces it.
///
/// Which is what a rename does, and what a placement means to do: the decision
/// about whether this device may write here was made before any byte was written
/// (spec: EP-10, EP-11), so by the time the rename happens the only question
/// left is whether the file becomes visible in one step. A capability that
/// refused instead would leave the flow with a verified file it could not put
/// anywhere.
pub async fn a_publish_replaces_what_stood_at_the_name(fixture: &DestinationsUnderTest) {
    let placed = fixture.root().join("spring.jpg");
    fixture.arrange().write_file(
        &placed,
        b"what was there before",
        Mtime::from_unix_seconds(1),
    );

    let destination = fixture
        .destinations()
        .reach(fixture.root(), &components(&["spring.jpg"]))
        .await
        .expect("reaching a place directly under the root must succeed");
    let mut scratch = destination
        .create(SCRATCH)
        .expect("creating a temporary file must succeed");
    scratch
        .write(CONTENT)
        .await
        .expect("writing the plaintext must succeed");
    let flushed = scratch.flush().await.expect("flushing must succeed");
    flushed.publish().expect("the rename must succeed");

    assert_eq!(
        fixture.arrange().content(&placed).as_deref(),
        Some(CONTENT),
        "the name carries what the placement wrote",
    );
}

/// A temporary file whose name is already taken is refused, and named as a
/// creation.
///
/// The scratch names a fetch draws are unique (see
/// [`scratch`](crate::scratch)), so this never happens by accident — which is
/// exactly why it must not be silently tolerated. Two writers sharing one
/// half-written file would each verify a hash over bytes the other interleaved,
/// so the create is exclusive and a taken name is a refusal.
pub async fn a_scratch_name_that_is_taken_is_refused(fixture: &DestinationsUnderTest) {
    fixture.arrange().write_file(
        &fixture.root().join(SCRATCH),
        b"a temporary file some other run left",
        Mtime::from_unix_seconds(1),
    );

    let destination = fixture
        .destinations()
        .reach(fixture.root(), &components(&["spring.jpg"]))
        .await
        .expect("reaching a place directly under the root must succeed");

    let refused = destination
        .create(SCRATCH)
        .err()
        .expect("a name that is taken may not be opened");
    assert!(
        matches!(
            refused,
            DescentError::Io(ref cause) if matches!(cause.operation, LocalOperation::Creating)
        ),
        "the refusal says the file could not be created: {refused:?}",
    );
}
