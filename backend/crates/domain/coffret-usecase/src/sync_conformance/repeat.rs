use coffret_model::{EntryPath, Generation, Mtime};

use crate::sync::sync_folders;
use crate::sync_conformance::fixtures::{keys, map, request, spooled, touch, write, NEWER, OLDER};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// Syncing a folder nothing happened to commits nothing.
///
/// Not "commits an empty batch": a Journal record is a generation, and the
/// Library's head is where the first sync left it (spec: CP-1). The files are
/// reported as unchanged without being opened, because their length and
/// modification time are what this device last observed (spec: EP-10).
pub async fn an_unchanged_second_sync_commits_nothing(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    write(fixture.folder(), "a.jpg", b"the file's bytes").await;
    write(fixture.folder(), "b.jpg", b"another file").await;
    let first = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync must succeed");
    let head = first
        .commit
        .expect("two new files are worth a commit")
        .record
        .generation;
    assert_eq!(head, Generation::FIRST);

    let second = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a second sync of an untouched folder must succeed");

    assert!(second.commit.is_none(), "there was nothing to commit");
    assert!(second.added.is_empty());
    assert!(second.replaced.is_empty());
    assert!(second.deferred.is_empty());
    assert_eq!(second.unchanged, 2);

    let checkpoint = index
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("the first sync committed");
    assert_eq!(
        checkpoint.head_generation, head,
        "the Library's head is where the first sync left it",
    );
    assert_eq!(spooled(fixture.spool()).await, 0);
}

/// A file that was touched and not changed commits nothing, and stops being
/// read on every later run.
///
/// The cheap comparison says the file may have changed and the expensive one
/// settles it: the plaintext still hashes to what the Entry records, so nothing
/// is uploaded (spec: EP-10, PK-11). What does happen is that the device writes
/// down what it now sees, so the next run answers from the length and the
/// modification time again instead of opening the file a second time.
pub async fn a_touched_file_with_equal_content_commits_nothing(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(fixture.folder(), "a.jpg", b"the file's bytes").await;
    touch(&path, OLDER);
    sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync must succeed")
        .commit
        .expect("a new file is worth a commit");

    touch(&path, NEWER);
    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a sync after a touch must succeed");

    assert!(
        outcome.commit.is_none(),
        "equal content is not a change, however the stamp moved",
    );
    assert!(outcome.added.is_empty());
    assert!(outcome.replaced.is_empty());
    assert_eq!(outcome.unchanged, 1);

    let local = index
        .local_entry_at(&EntryPath::new("a.jpg"))
        .await
        .expect("asking the Index for a local row must succeed")
        .expect("this device placed the file");
    assert_eq!(
        local.observation.mtime,
        Mtime::from_unix_seconds(NEWER as i64),
        "the device wrote down what it now sees, so the next run reads nothing",
    );
}
