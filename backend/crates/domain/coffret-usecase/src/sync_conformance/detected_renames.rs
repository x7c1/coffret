//! What a sync makes of a file renamed on disk (spec: EP-10, FM-15).
//!
//! A file this device materialized that is gone, and a file that appeared
//! under the same mapped root holding exactly its Entry's content, are one file
//! renamed — where nothing else the scan knows of holds that content. The pair
//! is carried into the Library as a rename of the Entry: no Container is put,
//! the record follows the file, and nothing is reported deleted locally. Every
//! case where which file became which is a guess pairs nothing, and the files
//! are what they were before a sync noticed renames at all: the new one
//! uploaded, the gone one reported deleted locally.

use std::path::{Path, PathBuf};

use coffret_model::{ContainerId, ContainerKind, Mtime};

use crate::device_state::LocalEntryState;
use crate::entry_paths::entry_path;
use crate::index::Index;
use crate::index_error::IndexError;
use crate::relocate::Relocation;
use crate::rename::{RenameRefusal, RenamedPath};
use crate::sync::{sync_folders, Surfaced, SyncError, SyncOutcome, SyncResult};
use crate::sync_conformance::counting_store::CountingStore;
use crate::sync_conformance::fixtures::{
    keys, map, map_at, observed, plant, request, write, OLDER,
};
use crate::sync_conformance::refusing_index::RefusingIndex;
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// What the catalog and this device's record said of one path before a case
/// renamed its file: the Container holding the Entry there, and the length and
/// modification time the record observed.
type Standing = (Option<ContainerId>, Option<(u64, Mtime)>);

/// One sync run through `index`, and how many Containers it put.
async fn sync_through(
    fixture: &SyncUnderTest,
    index: &dyn Index,
    run: i64,
) -> (SyncResult<SyncOutcome>, usize) {
    let keys = keys();
    let counting = CountingStore::around(fixture.store());
    let outcome = sync_folders(request(&counting, index, &keys, fixture.fs(), run)).await;
    (outcome, counting.containers_put())
}

/// One sync run, which the case expects to succeed, and how many Containers it
/// put.
async fn sync(fixture: &SyncUnderTest, run: i64) -> (SyncOutcome, usize) {
    let (outcome, puts) = sync_through(fixture, fixture.index(), run).await;
    let outcome = outcome.unwrap_or_else(|error| panic!("sync run {run} must succeed: {error}"));
    (outcome, puts)
}

/// Writes `files` into the fixture's folder, mapped onto the Library root, and
/// carries them in with a first sync.
async fn carry_in(fixture: &SyncUnderTest, files: &[(&str, &[u8])]) {
    map(fixture, None).await;
    for (relative, content) in files {
        write(fixture.fs(), fixture.folder(), relative, content);
    }
    let (first, _) = sync(fixture, 1).await;
    assert_eq!(first.added.len(), files.len(), "every file is carried in");
}

/// Renames a file on the device's disk the way a person does: the same bytes
/// and the same modification time, at another name.
fn rename_on_disk(fixture: &SyncUnderTest, root: &Path, from: &str, to: &str) {
    let from = root.join(from);
    let content = fixture
        .fs()
        .content(&from)
        .expect("a case renames a file it wrote");
    let (_, mtime) = observed(fixture.fs(), &from);
    fixture.fs().remove_file(&from);
    let to = root.join(to);
    fixture.fs().write_file(&to, &content);
    fixture.fs().set_mtime(&to, mtime.as_unix_seconds());
}

/// One rename an outcome reports.
fn renamed(from: &str, to: &str) -> RenamedPath {
    RenamedPath {
        from: entry_path(from),
        to: entry_path(to),
    }
}

/// The paths a run reported deleted locally.
fn deleted_locally(outcome: &SyncOutcome) -> Vec<&str> {
    outcome
        .surfaced
        .iter()
        .filter_map(|surfaced| match surfaced {
            Surfaced::DeletedLocally { path } => Some(path.as_str()),
            _ => None,
        })
        .collect()
}

/// Asserts that every `(from, to)` was carried in as a rename and nothing else
/// happened: no Container put, nothing reported, the Entry at the new path the
/// one that stood at the old, and the record re-keyed with what it observed.
async fn carried_as_renames(
    fixture: &SyncUnderTest,
    renames: &[(&str, &str)],
    before: Vec<Standing>,
    outcome: &SyncOutcome,
    puts: usize,
) {
    assert_eq!(puts, 0, "no Container is put for a renamed file");
    assert!(outcome.added.is_empty(), "nothing is added");
    assert!(outcome.replaced.is_empty(), "nothing is replaced");
    assert!(outcome.commit.is_some(), "the renames are committed");
    assert!(outcome.surfaced.is_empty(), "{:?}", outcome.surfaced);
    let mut expected: Vec<RenamedPath> =
        renames.iter().map(|(from, to)| renamed(from, to)).collect();
    expected.sort_by(|left, right| left.from.cmp(&right.from));
    assert_eq!(outcome.renamed, expected);

    let index = fixture.index();
    for ((from, to), (container, observation)) in renames.iter().zip(before) {
        assert!(
            index
                .entry_at(&entry_path(*from))
                .await
                .expect("asking the catalog must succeed")
                .is_none(),
            "no Entry is left at {from}",
        );
        let entry = index
            .entry_at(&entry_path(*to))
            .await
            .expect("asking the catalog must succeed")
            .unwrap_or_else(|| panic!("an Entry stands at {to}"));
        assert_eq!(Some(entry.container_id), container, "the same Entry");
        assert!(
            index
                .local_entry_at(&entry_path(*from))
                .await
                .expect("reading a row must succeed")
                .is_none(),
            "no record is left at {from}",
        );
        let row = index
            .local_entry_at(&entry_path(*to))
            .await
            .expect("reading a row must succeed")
            .unwrap_or_else(|| panic!("the record follows the file to {to}"));
        assert_eq!(row.state, LocalEntryState::Present);
        assert_eq!(row.relocate_to, None, "nothing waits to be relocated");
        assert_eq!(
            Some((row.observation.size, row.observation.mtime)),
            observation,
            "the observation is the one recorded before",
        );
    }
}

/// What the catalog and the records say of `paths` before a case renames them.
async fn standing(fixture: &SyncUnderTest, paths: &[&str]) -> Vec<Standing> {
    let mut standing = Vec::with_capacity(paths.len());
    for path in paths {
        let entry = fixture
            .index()
            .entry_at(&entry_path(*path))
            .await
            .expect("asking the catalog must succeed");
        let row = fixture
            .index()
            .local_entry_at(&entry_path(*path))
            .await
            .expect("reading a row must succeed");
        standing.push((
            entry.map(|entry| entry.container_id),
            row.map(|row| (row.observation.size, row.observation.mtime)),
        ));
    }
    standing
}

/// Asserts that a run carried nothing as a rename, uploaded `added` files and
/// reported `deleted` as deleted locally.
fn paired_nothing(outcome: &SyncOutcome, puts: usize, added: usize, deleted: &[&str]) {
    assert!(outcome.renamed.is_empty(), "{:?}", outcome.renamed);
    assert_eq!(outcome.added.len(), added, "the new files are uploaded");
    assert_eq!(puts, added, "one Container per new file");
    assert_eq!(deleted_locally(outcome), deleted);
}

/// A file renamed within its mapped root is one rename: no Container put, its
/// record re-keyed with what it observed, nothing reported deleted locally, and
/// the next run finds the file unchanged at its Entry's own path
/// (spec: EP-10).
pub async fn a_file_renamed_in_its_root_is_carried_as_a_rename(fixture: &SyncUnderTest) {
    carry_in(
        fixture,
        &[
            ("a.jpg", b"the file's bytes"),
            ("b.jpg", b"a file that stays"),
        ],
    )
    .await;
    let before = standing(fixture, &["a.jpg"]).await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "c.jpg");

    let (outcome, puts) = sync(fixture, 2).await;

    carried_as_renames(fixture, &[("a.jpg", "c.jpg")], before, &outcome, puts).await;
    assert_eq!(outcome.unchanged, 1, "the file that stayed is unchanged");
    let record = outcome.commit.as_ref().expect("committed").record.clone();
    assert_eq!(record.renames().len(), 1, "one rename in the record");
    assert!(record.additions().is_empty(), "and no addition");

    let (again, puts) = sync(fixture, 3).await;
    assert_eq!(puts, 0);
    assert!(again.commit.is_none(), "nothing is left to commit");
    assert!(again.surfaced.is_empty(), "{:?}", again.surfaced);
    assert_eq!(again.unchanged, 2, "the renamed file is its Entry's copy");
}

/// A file renamed into a sub-folder of its root is one rename too
/// (spec: EP-10).
pub async fn a_file_renamed_into_a_sub_folder_is_carried_as_a_rename(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("inbox/a.jpg", b"the file's bytes")]).await;
    let before = standing(fixture, &["inbox/a.jpg"]).await;
    rename_on_disk(
        fixture,
        fixture.folder(),
        "inbox/a.jpg",
        "albums/2026/a.jpg",
    );

    let (outcome, puts) = sync(fixture, 2).await;

    carried_as_renames(
        fixture,
        &[("inbox/a.jpg", "albums/2026/a.jpg")],
        before,
        &outcome,
        puts,
    )
    .await;
}

/// A folder renamed on disk is its files renamed, each pairing one to one with
/// its own Entry (spec: EP-10).
pub async fn a_renamed_folder_is_carried_file_by_file(fixture: &SyncUnderTest) {
    carry_in(
        fixture,
        &[
            ("books/vol1.txt", b"volume one"),
            ("books/vol2.txt", b"volume two, longer"),
            ("books/sub/vol3.txt", b"volume three, the longest"),
        ],
    )
    .await;
    let renames = [
        ("books/sub/vol3.txt", "shelf/sub/vol3.txt"),
        ("books/vol1.txt", "shelf/vol1.txt"),
        ("books/vol2.txt", "shelf/vol2.txt"),
    ];
    let before = standing(fixture, &renames.map(|(from, _)| from)).await;
    for (from, to) in renames {
        rename_on_disk(fixture, fixture.folder(), from, to);
    }

    let (outcome, puts) = sync(fixture, 2).await;

    carried_as_renames(fixture, &renames, before, &outcome, puts).await;
}

/// Two files with the same content, one of them renamed: the new file could as
/// well be a copy of the one that stayed, so nothing pairs (spec: EP-10).
pub async fn a_rename_beside_an_identical_file_pairs_nothing(fixture: &SyncUnderTest) {
    carry_in(
        fixture,
        &[("a.jpg", b"same bytes"), ("b.jpg", b"same bytes")],
    )
    .await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "c.jpg");

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 1, &["a.jpg"]);
}

/// Two gone files with the same content and one new file holding it: which of
/// them it was is a guess, so nothing pairs (spec: EP-10).
pub async fn two_gone_files_with_one_content_pair_nothing(fixture: &SyncUnderTest) {
    carry_in(
        fixture,
        &[("a.jpg", b"same bytes"), ("b.jpg", b"same bytes")],
    )
    .await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "c.jpg");
    fixture.fs().remove_file(&fixture.folder().join("b.jpg"));

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 1, &["a.jpg", "b.jpg"]);
}

/// One gone file and two new files holding its content: which one it became is
/// a guess, so nothing pairs (spec: EP-10).
pub async fn one_gone_file_and_two_identical_new_files_pair_nothing(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "c.jpg");
    write(fixture.fs(), fixture.folder(), "d.jpg", b"the file's bytes");

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 2, &["a.jpg"]);
}

/// A new file holding the content of an Entry this device never materialized
/// is new: a file this device never placed is never gone (spec: EP-10).
pub async fn a_new_file_like_an_entry_never_placed_here_is_new(fixture: &SyncUnderTest) {
    map(fixture, None).await;
    let keys = keys();
    plant(
        fixture.store(),
        fixture.index(),
        &keys,
        ContainerKind::OneFile,
        "a.jpg",
        b"another device's bytes",
        Mtime::from_unix_seconds(OLDER),
        false,
    )
    .await;
    write(
        fixture.fs(),
        fixture.folder(),
        "c.jpg",
        b"another device's bytes",
    );

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 1, &[]);
}

/// A new file whose content is not the gone file's is new, and the gone file
/// is deleted locally (spec: EP-10).
pub async fn a_new_file_with_other_content_is_new(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    fixture.fs().remove_file(&fixture.folder().join("a.jpg"));
    // The same length, so it is read and its hash is what tells them apart.
    write(fixture.fs(), fixture.folder(), "c.jpg", b"the file's BYTES");

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 1, &["a.jpg"]);
}

/// A file edited and renamed is a new file and a deletion, as before
/// (spec: EP-10).
pub async fn an_edited_and_renamed_file_is_new_and_its_old_path_deleted(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "c.jpg");
    fixture
        .fs()
        .write_file(&fixture.folder().join("c.jpg"), b"the file's bytes, edited");

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 1, &["a.jpg"]);
}

/// A file renamed from one mapped root into another pairs nothing: the two
/// mappings are two folders, and a rename never crosses them (spec: EP-9,
/// EP-10).
pub async fn a_rename_across_two_mapped_roots_pairs_nothing(fixture: &SyncUnderTest) {
    let one: PathBuf = fixture.folder().join("one");
    let two: PathBuf = fixture.folder().join("two");
    write(fixture.fs(), &one, "a.jpg", b"the file's bytes");
    fixture.fs().create_dir(&two);
    map_at(fixture, None, &one).await;
    map_at(fixture, Some("two"), &two).await;
    let (first, _) = sync(fixture, 1).await;
    assert_eq!(first.added.len(), 1);

    let content = fixture
        .fs()
        .content(&one.join("a.jpg"))
        .expect("the file is there");
    fixture.fs().remove_file(&one.join("a.jpg"));
    write(fixture.fs(), &two, "a.jpg", &content);

    let (outcome, puts) = sync(fixture, 2).await;

    paired_nothing(&outcome, puts, 1, &["a.jpg"]);
}

/// A pair whose rename the Library refuses falls back to what the files would
/// have been — the new one uploaded, the gone one deleted locally — and the run
/// says why the rename was not carried (spec: EP-10, FM-15).
pub async fn a_refused_rename_falls_back_with_a_finding(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    // An Entry another device placed at `x`, which this device never had: a
    // file at `x/y` would make `x` both a file and a folder.
    let keys = keys();
    plant(
        fixture.store(),
        fixture.index(),
        &keys,
        ContainerKind::OneFile,
        "x",
        b"another device's file",
        Mtime::from_unix_seconds(OLDER),
        false,
    )
    .await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "x/y");

    let (outcome, puts) = sync(fixture, 2).await;

    assert!(outcome.renamed.is_empty(), "{:?}", outcome.renamed);
    assert_eq!(outcome.added.len(), 1, "the new file is uploaded");
    assert_eq!(puts, 1);
    assert_eq!(deleted_locally(&outcome), ["a.jpg"]);
    let refused: Vec<_> = outcome
        .surfaced
        .iter()
        .filter_map(|surfaced| match surfaced {
            Surfaced::RenameRefused {
                path,
                from,
                refusal,
            } => Some((path.as_str(), from.as_str(), refusal)),
            _ => None,
        })
        .collect();
    match refused.as_slice() {
        [("x/y", "a.jpg", RenameRefusal::FileAndFolder { path })] => {
            assert_eq!(path.as_str(), "x");
        }
        other => panic!("expected the refused rename reported, got {other:?}"),
    }
}

/// A run interrupted after its rename landed and before the record followed
/// the file leaves the record at the old path, waiting to be relocated to where
/// the file already stands. The next run re-keys it without renaming anything
/// on disk: nothing is uploaded, and the old path is not reported deleted
/// locally (spec: EP-10).
pub async fn a_detected_rename_interrupted_before_its_record_follows_is_finished_by_the_next_run(
    fixture: &SyncUnderTest,
) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    let before = standing(fixture, &["a.jpg"]).await;
    rename_on_disk(fixture, fixture.folder(), "a.jpg", "c.jpg");

    let (interrupted, puts) = sync_through(
        fixture,
        &RefusingIndex::refusing_relocations(fixture.index()),
        2,
    )
    .await;
    assert!(
        matches!(
            interrupted,
            Err(SyncError::Index(IndexError::Backend { .. }))
        ),
        "the run stops where the record could not follow: {interrupted:?}"
    );
    assert_eq!(puts, 0, "no Container is put for a renamed file");
    let index = fixture.index();
    let entry = index
        .entry_at(&entry_path("c.jpg"))
        .await
        .expect("asking the catalog must succeed")
        .expect("the rename has landed");
    assert_eq!(Some(entry.container_id), before[0].0, "the same Entry");
    assert_eq!(
        index
            .local_entry_at(&entry_path("a.jpg"))
            .await
            .expect("reading a row must succeed")
            .expect("the record has not followed")
            .relocate_to,
        Some(entry_path("c.jpg")),
    );

    let (next, puts) = sync(fixture, 3).await;

    assert_eq!(puts, 0, "the file is not uploaded as new");
    assert!(next.added.is_empty(), "nothing is added");
    assert!(next.renamed.is_empty(), "the rename was already committed");
    assert!(next.commit.is_none(), "nothing is left to commit");
    assert!(next.surfaced.is_empty(), "{:?}", next.surfaced);
    match next.relocations.as_slice() {
        [Relocation::Relocated { from, to }] => {
            assert_eq!((from.as_str(), to.as_str()), ("a.jpg", "c.jpg"));
        }
        other => panic!("expected the record re-keyed to where the file stands, got {other:?}"),
    }
    assert!(index
        .local_entry_at(&entry_path("a.jpg"))
        .await
        .expect("reading a row must succeed")
        .is_none());
    let row = index
        .local_entry_at(&entry_path("c.jpg"))
        .await
        .expect("reading a row must succeed")
        .expect("the record follows the file");
    assert_eq!(row.relocate_to, None);
    assert_eq!(
        Some((row.observation.size, row.observation.mtime)),
        before[0].1,
        "the observation is the one recorded before",
    );
    assert_eq!(next.unchanged, 1, "the file is its Entry's copy");
}

/// A gone file that pairs with nothing in its scan keeps its record and is
/// reported deleted locally, so a later scan that finds its partner still
/// pairs them: a file taken out of the mapped folder and brought back under
/// another name is a rename, not a second upload (spec: EP-10).
pub async fn a_gone_file_paired_by_a_later_scan_is_carried_as_a_rename(fixture: &SyncUnderTest) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    let before = standing(fixture, &["a.jpg"]).await;
    let from = fixture.folder().join("a.jpg");
    let content = fixture
        .fs()
        .content(&from)
        .expect("the case wrote the file");
    let (_, mtime) = observed(fixture.fs(), &from);
    fixture.fs().remove_file(&from);

    let (gone, puts) = sync(fixture, 2).await;

    paired_nothing(&gone, puts, 0, &["a.jpg"]);
    assert!(gone.commit.is_none(), "nothing is committed");
    assert!(
        fixture
            .index()
            .local_entry_at(&entry_path("a.jpg"))
            .await
            .expect("reading a row must succeed")
            .is_some(),
        "the record is kept",
    );

    let to = fixture.folder().join("c.jpg");
    fixture.fs().write_file(&to, &content);
    fixture.fs().set_mtime(&to, mtime.as_unix_seconds());

    let (outcome, puts) = sync(fixture, 3).await;

    carried_as_renames(fixture, &[("a.jpg", "c.jpg")], before, &outcome, puts).await;
}
