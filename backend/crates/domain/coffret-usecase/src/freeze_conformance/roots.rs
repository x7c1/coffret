use std::path::{Path, PathBuf};

use crate::conformance_library::Library;
use crate::freeze::{FreezeOutcome, RootUnavailable, UnavailableRoot};
use crate::freeze_conformance::counting_store::CountingStore;
use crate::freeze_conformance::fixtures::{
    another_filesystem, container_handle, filler, freeze, freeze_against, keys, map, map_with,
    write, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;

/// Enough files, on both sides of the target, that the folder is cut more than
/// once.
fn files() -> Vec<(String, Vec<u8>)> {
    (0..9)
        .map(|index| {
            (
                format!("2026/{index:03}.jpg"),
                filler(50 + (index * 41) % 130, 0x40 + index as u8),
            )
        })
        .collect()
}

/// A freeze over a mapped root that is not there says so, rather than looking
/// like a run with nothing to do.
///
/// This is the silent half of the same bug. A freeze over an unmounted root
/// walks zero files, so it selects nothing, cuts no segment, commits nothing,
/// and returns successfully — which is exactly what the second freeze of an
/// already-packed folder looks like (spec: PK-2). The user cannot tell "already
/// packed" from "the disk is not plugged in", and silence about a folder that is
/// not being packed is the one outcome the rule forbids (spec: PK-14). So the
/// case asserts both answers side by side: the same empty outcome twice, with
/// [`FreezeOutcome::unavailable`] the only thing that differs (spec: EP-12).
pub async fn a_missing_mapped_root_is_surfaced_by_a_freeze(fixture: &FreezeUnderTest) {
    let keys = keys();
    let root = source_root(fixture).await;

    let first = freeze(fixture, &keys, TARGET, 1).await;
    assert!(first.packs.len() > 1, "the folder is cut more than once");
    assert_eq!(first.frozen_entries(), files().len());
    assert!(first.unavailable.is_empty(), "the root was there");
    let library = Library::read(fixture.store()).await;
    for pack in &first.packs {
        assert!(
            library.holds_container(pack.container_id),
            "the Pack's object is on Storage",
        );
    }

    // The ordinary second run over the intact folder: every file already packed,
    // so nothing to do and nothing wrong (spec: PK-2).
    let intact = freeze(fixture, &keys, TARGET, 2).await;
    assert_nothing_done(&intact);
    assert_eq!(intact.packed_already, files().len());
    assert!(
        intact.unavailable.is_empty(),
        "a folder that is simply already packed has no unavailable root",
    );

    tokio::fs::remove_dir_all(&root)
        .await
        .expect("removing a mapped root must succeed");

    let unplugged = freeze(fixture, &keys, TARGET, 3).await;
    assert_nothing_done(&unplugged);
    assert_eq!(
        unplugged.packed_already, 0,
        "nothing under an unavailable root was even considered",
    );
    assert_eq!(
        unplugged.unavailable,
        vec![UnavailableRoot {
            prefix: None,
            local_root: root,
            reason: RootUnavailable::Missing,
        }],
        "the one field that distinguishes an unplugged disk from an already-packed folder",
    );
}

/// A freeze over a root that is empty on a filesystem the mapping does not
/// record says so too, and leaves the Packs alone.
///
/// The same finding by the other route: an unmounted mount point lists as an
/// empty directory, so the walk succeeds and finds nothing (spec: EP-12). What a
/// freeze must not do about it is anything at all — an unavailable root
/// contributes no candidate, so it can neither absorb nor remove, and the Packs
/// an earlier run built stay byte-for-byte where they are (spec: PK-2). That is
/// a claim about an absence, so the case names the objects the run touched
/// rather than reading it off the outcome.
pub async fn an_empty_root_on_another_filesystem_is_surfaced_by_a_freeze(
    fixture: &FreezeUnderTest,
) {
    let keys = keys();
    let root = source_root(fixture).await;

    let first = freeze(fixture, &keys, TARGET, 1).await;
    assert!(first.packs.len() > 1, "the folder is cut more than once");

    // What an unmount leaves behind: the recorded identity is no longer the one
    // the root stands on, and the root holds nothing.
    map_with(fixture.source(), None, &root, Some(another_filesystem())).await;
    empty_the_root(&root).await;

    let library = Library::read(fixture.store()).await;
    let counting = CountingStore::around(fixture.store());
    let outcome = freeze_against(&counting, fixture, &keys, TARGET, 2).await;

    assert_nothing_done(&outcome);
    assert_eq!(
        outcome.packed_already, 0,
        "nothing under an unavailable root was even considered",
    );
    assert_eq!(
        outcome.unavailable,
        vec![UnavailableRoot {
            prefix: None,
            local_root: root,
            reason: RootUnavailable::AnotherFilesystem,
        }],
    );
    assert_eq!(counting.writes(), 0, "the run wrote nothing at all");
    for pack in &first.packs {
        assert!(
            !counting.wrote(pack.container_id),
            "an unavailable root gives a freeze nothing to rewrite (spec: PK-2)",
        );
        let object = container_handle(fixture.store(), pack.container_id).await;
        assert!(
            !counting.read_object(&object),
            "and nothing to read either (spec: PK-1)",
        );
        assert!(
            library.holds_container(pack.container_id),
            "the Pack's object is where the first run left it",
        );
    }
}

/// A subdirectory of the source folder, mapped at the Library root and holding
/// the case's files.
///
/// A subdirectory rather than the fixture's own folder, because a case that
/// removes a mapped root must not take the fixture's directory with it.
async fn source_root(fixture: &FreezeUnderTest) -> PathBuf {
    let root = fixture.source_folder().join("photographs");
    map(fixture.source(), None, &root).await;
    for (relative, content) in files() {
        write(&root, &relative, &content).await;
    }
    root
}

/// Removes everything under a mapped root, leaving the root itself there and
/// holding no directory entry at all — which is what an unmounted mount point
/// looks like.
async fn empty_the_root(root: &Path) {
    let mut listing = tokio::fs::read_dir(root)
        .await
        .expect("listing a mapped root must succeed");
    while let Some(entry) = listing
        .next_entry()
        .await
        .expect("listing a mapped root must succeed")
    {
        tokio::fs::remove_dir_all(entry.path())
            .await
            .expect("removing a folder must succeed");
    }
}

/// The empty answer a freeze gives when it has nothing to pack.
fn assert_nothing_done(outcome: &FreezeOutcome) {
    assert!(outcome.packs.is_empty(), "nothing was packed");
    assert!(outcome.absorbed.is_empty(), "nothing was absorbed");
    assert!(outcome.surfaced.is_empty(), "nothing else was surfaced");
    assert!(outcome.commit.is_none(), "nothing was committed");
}
