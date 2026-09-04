use coffret_model::{ContainerKind, Generation};

use crate::conformance_library::Library;
use crate::device_state::Mapping;
use crate::entry_paths::entry_path;
use crate::sync::sync_folders;
use crate::sync_conformance::fixtures::{
    born, keys, map, master_key, observed, request, spooled, write,
};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// The first sync of a folder puts every file in the Library, and another
/// device can open them.
///
/// This is the whole point of the path, so it is asserted the whole way round
/// rather than from what the run reported. Each file becomes a one-file
/// Container of its own (spec: PK-15), the batch commits as one Journal record
/// (spec: CP-1), and the ciphertext that is actually on Storage — fetched back,
/// opened through the envelope the committed Keyring maps it to, and decoded —
/// carries the bytes that were on disk
/// (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9, FM-14, KL-7).
///
/// The device's own bookkeeping is checked alongside, because uploading a file
/// is exactly what materializing it means: after this run the device may report
/// these files as deleted if they later go missing, which it could not do for
/// Entries it had never placed (spec: EP-10).
pub async fn a_first_sync_commits_every_file_and_they_decode(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let first = b"the first file's bytes".as_slice();
    let second = b"a second file, in a folder below".as_slice();
    let first_path = write(fixture.folder(), "a.jpg", first).await;
    write(fixture.folder(), "below/b.png", second).await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync of a folder must succeed");

    assert_eq!(
        outcome.added.len(),
        2,
        "one Container per file (spec: PK-15)"
    );
    assert!(outcome.replaced.is_empty());
    assert!(outcome.surfaced.is_empty(), "nothing was surfaced");
    assert_eq!(outcome.unchanged, 0);

    let commit = outcome.commit.expect("two new files are worth a commit");
    assert_eq!(commit.record.generation, Generation::FIRST);
    assert_eq!(commit.record.additions.len(), 2);
    assert!(commit.record.removals.is_empty());
    for addition in &commit.record.additions {
        assert_eq!(addition.container.kind, ContainerKind::OneFile);
        assert_eq!(addition.entries.len(), 1);
    }

    let library = Library::read(store).await;
    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the file this run uploaded is current");

    let container = library
        .open(store, &commit.record, location.container_id, &master_key())
        .await;
    assert_eq!(container.kind, ContainerKind::OneFile);
    assert_eq!(container.entries.len(), 1);
    assert_eq!(
        container.entries[0].content, first,
        "what another device decodes is the file that was on disk",
    );
    assert_eq!(container.entries[0].metadata.path.as_str(), "a.jpg");

    let below = index
        .entry_at(&entry_path("below/b.png"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("a file in a folder below the root is current too");
    let container = library
        .open(store, &commit.record, below.container_id, &master_key())
        .await;
    assert_eq!(container.entries[0].content, second);

    // Uploading a file is placing it: the device may now report it as deleted
    // if it goes missing, which it could not for an Entry it never held
    // (spec: EP-10).
    let (size, mtime) = observed(&first_path).await;
    let local = index
        .local_entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a local row must succeed")
        .expect("this device placed the file, so it has a row for it");
    assert_eq!(local.observation.size, size);
    assert_eq!(local.observation.mtime, mtime);

    assert_eq!(
        spooled(fixture.spool()).await,
        0,
        "a committed batch leaves no ciphertext on the device",
    );
    assert!(
        index
            .pending_uploads()
            .await
            .expect("asking the Index for pending uploads must succeed")
            .is_empty(),
        "a committed Container is no longer a candidate for cleanup (spec: OC-2)",
    );
}

/// A file the mapping puts under a top-level component lands there (spec: EP-9).
///
/// The same folder, mapped at a prefix rather than at the Library root, so the
/// Entry Paths the scan derives are the mapping's and not the disk's.
pub async fn a_mapped_prefix_decides_where_a_file_lands(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, Some("albums")).await;

    write(fixture.folder(), "2026/spring.jpg", b"a photo").await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a sync of a mapped subtree must succeed");
    assert_eq!(outcome.added.len(), 1);

    assert!(
        index
            .entry_at(&entry_path("albums/2026/spring.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the mapping's prefix is the first component of the Entry Path",
    );
    assert!(
        index
            .entry_at(&entry_path("2026/spring.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "the local path below the root is not an Entry Path of its own",
    );
}

/// One file name in the two spellings a filesystem may hand a scan back.
///
/// `é` is a single code point composed and `e` followed by a combining acute
/// decomposed. The two render identically and are different byte sequences, and
/// which one a scan reads back is the filesystem's business rather than the
/// user's: one hands back the spelling a name was written with, and another
/// decomposes every name it stores and hands that back instead. An Entry Path is
/// in the composed form either way (spec: EP-1).
const COMPOSED: &str = "caf\u{e9}.jpg";
const DECOMPOSED: &str = "cafe\u{301}.jpg";

/// A local file whose name is decomposed lands at the composed Entry Path, and
/// stays there (spec: EP-1).
///
/// The scan is the boundary where an operating system's own text becomes an
/// Entry Path, so the scan is what owes NFC. The catalog compares the bytes it
/// is given and folds nothing together (spec: EP-3), so a name carried through
/// as read would put one file at two Library positions depending on which device
/// backed it up, and neither device would find the other's.
///
/// The second run is half the case. A filesystem that kept the decomposed name
/// hands the very same spelling back, and the run has to recognize the file it
/// already committed rather than commit it again under the composed name it used
/// the first time — which is the shape a fetch meets too, having written the
/// composed name itself (spec: EP-10).
pub async fn an_nfd_local_name_becomes_an_nfc_entry_path(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    // Written under the decomposed name, whatever the filesystem then does with
    // it. A filesystem that composes names on the way in is not a hole in the
    // case: either spelling has to reach the Library composed, and one of the
    // two is what this machine's disk will hand the scan back.
    write(fixture.folder(), DECOMPOSED, b"a photo of a cafe").await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync of a decomposed name must succeed");
    assert_eq!(outcome.added.len(), 1);

    let commit = outcome.commit.expect("a new file is worth a commit");
    let addition = commit
        .record
        .additions
        .first()
        .expect("the run committed one Container");
    let entry = addition
        .entries
        .first()
        .expect("a one-file Container holds one Entry");
    assert_eq!(
        entry.path.as_str(),
        COMPOSED,
        "what another device reads out of the Journal is the composed spelling",
    );

    assert!(
        index
            .entry_at(&entry_path(COMPOSED))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the file stands at the composed Entry Path",
    );
    assert_eq!(
        index
            .entries_under(None)
            .await
            .expect("reading the whole catalog must succeed")
            .len(),
        1,
        "the decomposed spelling is not a second Library position for it",
    );

    let second = sync_folders(request(store, index, &keys, fixture.spool(), 2))
        .await
        .expect("a second sync of the same folder must succeed");

    assert!(
        second.commit.is_none(),
        "the file the first run committed is the file on disk, however the disk spells it",
    );
    assert!(second.added.is_empty());
    assert!(second.replaced.is_empty());
    assert!(
        second.surfaced.is_empty(),
        "nothing went missing: the composed row and the scanned name are one path",
    );
    assert_eq!(second.unchanged, 1);
}

/// With both kinds of mapping present, the Library-root one walks the remainder
/// (spec: EP-9).
///
/// A top-level mapping represents its subtree, so a folder of the same name
/// under the root-mapped folder is not a second spelling of it: whatever sits
/// there is outside every mapping and stays out of the Library. A walk that
/// entered it anyway would either commit Entry Paths the other mapping owns —
/// from a folder the user pointed nothing at — or collide with the files that
/// mapping does hold.
pub async fn a_top_level_mapping_takes_its_subtree_from_the_root_mapping(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();

    // Two local roots, side by side, so that neither contains the other.
    let remainder = fixture.folder().join("everything");
    let albums = fixture.folder().join("photographs");
    for (prefix, local_root) in [
        (None, remainder.clone()),
        (Some(entry_path("albums")), albums.clone()),
    ] {
        index
            .set_mapping(Mapping {
                prefix,
                local_root,
                root_identity: None,
            })
            .await
            .expect("recording a mapping must succeed");
    }

    write(&remainder, "notes.txt", b"part of the remainder").await;
    write(
        &remainder,
        "albums/stray.jpg",
        b"under a folder nothing maps",
    )
    .await;
    write(&albums, "spring.jpg", b"a photo").await;

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a sync over both kinds of mapping must succeed");

    assert_eq!(
        outcome.added.len(),
        2,
        "the remainder and the mapped subtree, and nothing from inside `albums/`",
    );
    for path in ["notes.txt", "albums/spring.jpg"] {
        assert!(
            index
                .entry_at(&entry_path(path))
                .await
                .expect("asking the Index for a path must succeed")
                .is_some(),
            "{path} is what one of the two mappings stands for",
        );
    }
    assert!(
        index
            .entry_at(&entry_path("albums/stray.jpg"))
            .await
            .expect("asking the Index for a path must succeed")
            .is_none(),
        "the `albums/` mapping represents that subtree, so the root mapping does not walk it",
    );
}

/// A file's birth time reaches the Journal record, where the platform reports
/// one (spec: FM-9, FM-15).
///
/// The one moment a birth time can be captured is while the local file is still
/// in front of the device: nothing recovers it afterwards, and no fetch stamps
/// it onto a file it places (spec: EP-11). So a sync that dropped it would
/// lose the value for good rather than defer it, which is why this is asserted
/// against what the filesystem itself says rather than against a constant.
///
/// Both answers a platform can give are the case. Where a filesystem keeps
/// creation times, every committed Entry carries one; where it keeps none —
/// a tmpfs, an older platform — every committed Entry carries none, and the
/// field is absent rather than stood in for. A run that invented a birth time
/// on such a filesystem, or dropped one on a filesystem that has them, fails
/// the same assertion.
pub async fn a_walked_files_birth_time_reaches_the_record(fixture: &SyncUnderTest) {
    let store = fixture.store();
    let index = fixture.index();
    let keys = keys();
    map(fixture, None).await;

    let path = write(fixture.folder(), "a.jpg", b"a photo").await;
    let expected = born(&path);

    let outcome = sync_folders(request(store, index, &keys, fixture.spool(), 1))
        .await
        .expect("a first sync of a folder must succeed");
    let commit = outcome.commit.expect("a new file is worth a commit");
    let entry = commit.record.additions[0]
        .entries
        .first()
        .expect("a one-file Container holds one Entry")
        .clone();

    match expected {
        Some(btime) => assert_eq!(
            entry.btime,
            Some(btime),
            "the birth time this filesystem reports is what the record carries",
        ),
        None => assert_eq!(
            entry.btime, None,
            "a filesystem that keeps no birth time leaves the field absent, \
             rather than standing the epoch or the modification time in for it",
        ),
    }

    // And the catalog holds the same answer, so a device reading the Entry back
    // sees what the record committed rather than what its own clock would say.
    let location = index
        .entry_at(&entry_path("a.jpg"))
        .await
        .expect("asking the Index for a path must succeed")
        .expect("the file this run uploaded is current");
    assert_eq!(location.entry.btime, expected);
}
