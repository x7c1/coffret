use coffret_model::Mtime;

use crate::below_root_error::BelowRootError;
use crate::destinations_conformance::destinations_under_test::DestinationsUnderTest;
use crate::destinations_conformance::{components, registered, CONTENT};
use crate::kept_file::Aside;
use crate::local_operation::LocalOperation;

/// The name a case's scratch goes by while it is written.
const SCRATCH: &str = ".coffret-fetch-incoming-0123456789abcdef.part";

/// The scratch name a case keeps the file that stood at the name under.
const KEPT: &str = ".coffret-fetch-incoming-fedcba9876543210.part";

/// What stood at the name before a case's publish.
const BEFORE: &[u8] = b"what was there before";

/// Writes [`CONTENT`] through a scratch and publishes it at the name the
/// destination was reached for.
async fn publish(destination: &dyn crate::Destination) {
    let mut scratch = destination
        .create(SCRATCH)
        .expect("creating a scratch must succeed");
    scratch
        .write(CONTENT)
        .await
        .expect("writing the plaintext must succeed");
    let flushed = scratch.flush().await.expect("flushing must succeed");
    flushed.publish().expect("the rename must succeed");
}

/// A file kept aside before a publish wrote over it is put back over what
/// replaced it, and nothing is left under the scratch name (spec: LA-36,
/// EP-11).
///
/// What stopping an add that replaced a file of the person's rests on: the
/// second link holds the file the rename took the name from, and the rename
/// back is one step, so the name carries the one file or the other.
pub async fn a_file_kept_aside_is_put_back_over_what_replaced_it(fixture: &DestinationsUnderTest) {
    let expected = registered(fixture);
    let placed = fixture.root().join("spring.jpg");
    fixture
        .arrange()
        .write_file(&placed, BEFORE, Mtime::from_unix_seconds(1));
    let destination = fixture
        .destinations()
        .reach(
            fixture.root(),
            Some(&expected),
            &components(&["spring.jpg"]),
        )
        .await
        .expect("reaching a place directly under the root must succeed");

    let Aside::Kept(kept) = destination
        .keep_aside(KEPT)
        .expect("keeping the file aside must succeed")
    else {
        panic!("a file standing at the name is kept aside");
    };
    assert_eq!(
        fixture
            .arrange()
            .content(&fixture.root().join(KEPT))
            .as_deref(),
        Some(BEFORE),
        "the scratch name carries the file that stood there",
    );
    publish(destination.as_ref()).await;
    assert_eq!(fixture.arrange().content(&placed).as_deref(), Some(CONTENT));

    kept.put_back().expect("putting the file back must succeed");
    assert_eq!(
        fixture.arrange().content(&placed).as_deref(),
        Some(BEFORE),
        "the name carries the file it had before the publish",
    );
    assert!(
        !fixture.arrange().holds(&fixture.root().join(KEPT)),
        "nothing is left under the scratch name",
    );
}

/// A file kept aside and then let go leaves the published file at the name and
/// nothing under the scratch name, and letting it go twice is still success
/// (spec: LA-36, OC-8).
pub async fn a_file_kept_aside_and_let_go_leaves_only_what_was_published(
    fixture: &DestinationsUnderTest,
) {
    let expected = registered(fixture);
    let placed = fixture.root().join("spring.jpg");
    fixture
        .arrange()
        .write_file(&placed, BEFORE, Mtime::from_unix_seconds(1));
    let destination = fixture
        .destinations()
        .reach(
            fixture.root(),
            Some(&expected),
            &components(&["spring.jpg"]),
        )
        .await
        .expect("reaching a place directly under the root must succeed");

    let Aside::Kept(kept) = destination
        .keep_aside(KEPT)
        .expect("keeping the file aside must succeed")
    else {
        panic!("a file standing at the name is kept aside");
    };
    publish(destination.as_ref()).await;
    kept.remove()
        .expect("letting the kept file go must succeed");
    kept.remove().expect("letting it go again must succeed too");

    assert_eq!(fixture.arrange().content(&placed).as_deref(), Some(CONTENT));
    assert!(!fixture.arrange().holds(&fixture.root().join(KEPT)));
}

/// An empty place has nothing to keep aside, and nothing is made under the
/// scratch name (spec: LA-36).
pub async fn an_empty_place_has_nothing_to_keep_aside(fixture: &DestinationsUnderTest) {
    let expected = registered(fixture);
    let destination = fixture
        .destinations()
        .reach(
            fixture.root(),
            Some(&expected),
            &components(&["spring.jpg"]),
        )
        .await
        .expect("reaching a place directly under the root must succeed");

    assert!(matches!(destination.keep_aside(KEPT), Ok(Aside::Empty)));
    assert!(!fixture.arrange().holds(&fixture.root().join(KEPT)));
}

/// A file the filesystem will not link is left where it is and answered
/// unkept, as a refusal to link (spec: LA-36).
///
/// Skipped where the backend cannot arrange such a filesystem.
pub async fn a_link_the_filesystem_cannot_make_leaves_the_file_unkept(
    fixture: &DestinationsUnderTest,
) {
    if !fixture.arrange().refuse_links() {
        eprintln!("skipping: this backend cannot arrange a filesystem that makes no hard link");
        return;
    }
    let expected = registered(fixture);
    let placed = fixture.root().join("spring.jpg");
    fixture
        .arrange()
        .write_file(&placed, BEFORE, Mtime::from_unix_seconds(1));
    let destination = fixture
        .destinations()
        .reach(
            fixture.root(),
            Some(&expected),
            &components(&["spring.jpg"]),
        )
        .await
        .expect("reaching a place directly under the root must succeed");

    let aside = destination.keep_aside(KEPT);
    assert!(
        matches!(
            aside,
            Ok(Aside::Unkept(BelowRootError::Io(ref cause)))
                if matches!(cause.operation, LocalOperation::Linking)
        ),
        "the file is answered unkept, as a refusal to link: {aside:?}",
    );
    assert_eq!(fixture.arrange().content(&placed).as_deref(), Some(BEFORE));
    assert!(!fixture.arrange().holds(&fixture.root().join(KEPT)));
}
