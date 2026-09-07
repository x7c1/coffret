use coffret_model::Mtime;

use crate::destinations_conformance::components;
use crate::destinations_conformance::destinations_under_test::DestinationsUnderTest;
use crate::destinations_conformance::CONTENT;

/// The time a case's planted file carries (spec: FM-9).
const PLANTED: i64 = 1_500_000_000;

/// A place nothing stands at looks up to nothing, and not to a failure.
///
/// Both shapes of nothing, because the whole of EP-11's "the place is free"
/// verdict rests on them being one answer: the file itself absent, and a folder
/// on the way to it absent. An empty place is an empty place however few of the
/// folders above it have been made, and a capability that reported the second as
/// a refusal would leave the selection unable to tell "there is nothing here"
/// from "the disk would not answer".
pub async fn a_look_at_an_empty_place_finds_nothing(fixture: &DestinationsUnderTest) {
    let absent_file = fixture
        .destinations()
        .look_up(fixture.root(), &components(&["spring.jpg"]))
        .await
        .expect("an empty place is a verdict and not a failure");
    assert!(
        absent_file.is_none(),
        "nothing stands at the name, so there is nothing to say anything about",
    );

    let absent_folder = fixture
        .destinations()
        .look_up(fixture.root(), &components(&["albums", "spring.jpg"]))
        .await
        .expect("a folder that was never made is a verdict too");
    assert!(
        absent_folder.is_none(),
        "nothing can stand at the file's path if the folder above it does not exist",
    );
}

/// A file that is there is reported with its length, its time, and the fact that
/// it is a file.
///
/// The three things a writer has to know before it may claim a path (spec: EP-10,
/// EP-11), and the length and the time are the cheap comparison a fetch makes
/// against what this device wrote down when it last placed a file there. A
/// capability that answered either of them from a second look, or from the
/// clock, would make that comparison say a file had changed when nobody touched
/// it.
pub async fn a_look_at_a_file_reports_its_size_and_time(fixture: &DestinationsUnderTest) {
    let placed = fixture.root().join("albums").join("spring.jpg");
    fixture
        .arrange()
        .write_file(&placed, CONTENT, Mtime::from_unix_seconds(PLANTED));

    let standing = fixture
        .destinations()
        .look_up(fixture.root(), &components(&["albums", "spring.jpg"]))
        .await
        .expect("stating a file that is there must succeed")
        .expect("the file is there");

    assert_eq!(standing.size, CONTENT.len() as u64);
    assert_eq!(standing.mtime, Mtime::from_unix_seconds(PLANTED));
    assert!(
        standing.is_file,
        "an ordinary file is a file, which is what makes it something a fetch \
         can recognize as its own materialization",
    );
}
