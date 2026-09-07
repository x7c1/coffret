use coffret_model::Mtime;

use crate::local_operation::LocalOperation;
use crate::mapped_roots_conformance::mapped_roots_under_test::MappedRootsUnderTest;

/// A mapped root that is not there probes to nothing, and not to a failure.
///
/// The whole of EP-12 rests on this being an answer rather than an error. A
/// capability that reported absence as a refusal would leave the walk above
/// reading an error kind to tell "the disk is unplugged" from "the process may
/// not look" — which is exactly the reading the capability exists to remove.
pub async fn a_missing_root_probes_to_nothing(fixture: &MappedRootsUnderTest) {
    let missing = fixture.dir().join("never-created");

    let probe = fixture
        .roots()
        .probe_root(&missing)
        .await
        .expect("a root that is not there is a verdict and not a failure");

    assert!(
        probe.is_none(),
        "nothing stands at the path, so there is no root to say anything about",
    );
}

/// A root that is there probes to something, and says what filesystem it stands
/// on.
///
/// The identity is what tells an unmounted mount point from a folder somebody
/// emptied, and a device that could not say it would be guarded by the
/// missing-root check alone (spec: EP-12). Whether it can say it is the
/// platform's answer and not the capability's, so the assertion about the value
/// is made only where a platform reports one.
pub async fn a_present_root_probes_to_an_identity(fixture: &MappedRootsUnderTest) {
    let root = fixture.dir().join("photographs");
    fixture.arrange().create_dir(&root);

    let probe = fixture
        .roots()
        .probe_root(&root)
        .await
        .expect("stating a folder that is there must succeed")
        .expect("the root is there");

    #[cfg(unix)]
    assert!(
        probe.identity.is_some(),
        "a Unix-like platform can always say which filesystem a folder stands on",
    );
    #[cfg(not(unix))]
    let _ = probe;
}

/// A root that is a regular file probes to something, and the listing below it
/// is what refuses it.
///
/// The third state a path can be in, and the one the missing-root verdict must
/// not swallow: the probe follows links and states what is there, a file is
/// there, so it answers `Some` like any other path that exists (spec: EP-12).
/// Answering `None` would tell the walk above that the disk is unplugged and
/// quietly stop backing that mapping up — the very reading the capability
/// exists to remove. What a mapped root that is not a folder actually refuses is
/// the listing, and that is where the run fails.
pub async fn probing_a_root_that_is_a_regular_file_answers_and_leaves_the_refusal_to_the_listing(
    fixture: &MappedRootsUnderTest,
) {
    let root = fixture.dir().join("photographs.jpg");
    fixture
        .arrange()
        .write_file(&root, b"a photo", Mtime::from_unix_seconds(1_600_000_000));

    let probe = fixture
        .roots()
        .probe_root(&root)
        .await
        .expect("something stands at the path, so stating it is not a refusal");
    assert!(
        probe.is_some(),
        "a file is there, and only absence is the verdict `None` stands for",
    );

    let refused = fixture
        .roots()
        .list_folder(&root)
        .await
        .expect_err("what stands at the root is a file and not a folder to list");
    assert!(
        matches!(refused.operation, LocalOperation::Listing),
        "the call that was refused was the listing, got {refused:?}",
    );
    assert_eq!(refused.path, root, "and it names the root it was about");
}
