//! What a sync and a freeze make of the files an operating system writes
//! beside the person's own (spec: EP-17): none of them becomes an Entry, none
//! is reported, and an Entry carried in at such a path before the rule is not
//! read as deleted locally.

use std::path::Path;

use coffret_model::EntryPath;

use crate::device_state::{BatchId, Mapping};
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, preview_freeze, FreezeRequest};
use crate::index::Index;
use crate::local_scan::NONE_PLACED;
use crate::sync::{sync_folders, SyncOutcome};
use crate::sync_conformance::fixtures::{at, keys, map, policy, request, spool_dir, write};
use crate::sync_conformance::sync_under_test::SyncUnderTest;

/// A Pack size the freeze here stays well under, so the book goes into one.
const FREEZE_TARGET: u64 = 1 << 20;

/// One sync run, which the case expects to succeed.
async fn sync(fixture: &SyncUnderTest, run: i64) -> SyncOutcome {
    let keys = keys();
    sync_folders(request(
        fixture.store(),
        fixture.index(),
        &keys,
        fixture.fs(),
        run,
    ))
    .await
    .unwrap_or_else(|error| panic!("sync run {run} must succeed: {error}"))
}

/// Every Entry Path the Library holds, in EP-3 order.
async fn entries(index: &dyn Index) -> Vec<EntryPath> {
    index
        .entries_under(None)
        .await
        .expect("listing the Index must succeed")
        .into_iter()
        .map(|location| location.path().clone())
        .collect()
}

/// A sync of a mapped folder holding what an operating system keeps — at the
/// root's top and deeper, files and folders — carries in only the person's
/// files and reports nothing about the rest; a freeze of the folder then packs
/// only those, and its preview counts what was left as it is (spec: EP-17).
pub async fn the_files_an_operating_system_keeps_are_neither_synced_nor_packed(
    fixture: &SyncUnderTest,
) {
    map(fixture, None).await;
    for (relative, content) in [
        ("book/page-001.jpg", b"the first page".as_slice()),
        ("book/page-002.jpg", b"the second page"),
        ("book/.DS_Store", b"finder"),
        ("book/._page-001.jpg", b"apple double"),
        ("book/Thumbs.db", b"explorer"),
        ("book/__MACOSX/book/._page-002.jpg", b"apple double"),
        (".DS_Store", b"finder"),
        (".Spotlight-V100/Store-V2/index", b"spotlight"),
    ] {
        write(fixture.fs(), fixture.folder(), relative, content);
    }

    let synced = sync(fixture, 1).await;

    assert_eq!(synced.added.len(), 2, "the two pages are carried in");
    assert!(
        synced.surfaced.is_empty(),
        "nothing is reported about what the system keeps: {:?}",
        synced.surfaced,
    );
    assert_eq!(
        entries(fixture.index()).await,
        [
            entry_path("book/page-001.jpg"),
            entry_path("book/page-002.jpg")
        ],
    );

    let book = entry_path("book");
    let preview = preview_freeze(
        fixture.index(),
        fixture.fs(),
        &NONE_PLACED,
        Some(&book),
        None,
    )
    .await
    .expect("the preview must succeed");
    assert_eq!(preview.files, 2, "the two pages would be packed");
    assert_eq!(
        preview.system_files, 3,
        "beside three files the system keeps"
    );
    assert_eq!(preview.system_folders, 1, "and one folder, not entered");
    assert_eq!(preview.not_here, 0);

    let keys = keys();
    let frozen = freeze_folder(
        FreezeRequest::new(
            fixture.store(),
            fixture.index(),
            &keys,
            fixture.fs(),
            fixture.fs(),
            spool_dir(),
            FREEZE_TARGET,
            BatchId::new("freeze-2"),
            at(2),
        )
        .under(book)
        .with_policy(policy()),
    )
    .await
    .unwrap_or_else(|error| panic!("the freeze must succeed: {error}"));

    assert_eq!(frozen.frozen_entries(), 2, "only the pages are packed");
    assert_eq!(
        entries(fixture.index()).await,
        [
            entry_path("book/page-001.jpg"),
            entry_path("book/page-002.jpg")
        ],
        "and nothing the system keeps became an Entry",
    );
}

/// An Entry carried in at a path the rule leaves out — here a volume's folder
/// taken deeper down, which a later mapping puts at its own root's top — is
/// not reported as deleted locally by a scan that no longer looks for its file
/// (spec: EP-17, EP-10).
pub async fn an_entry_at_a_path_the_rule_leaves_out_is_not_deleted_locally(
    fixture: &SyncUnderTest,
) {
    map(fixture, None).await;
    let inner = "albums/.Spotlight-V100/notes.txt";
    write(fixture.fs(), fixture.folder(), inner, b"the person's notes");
    write(fixture.fs(), fixture.folder(), "albums/page.jpg", b"a page");
    let first = sync(fixture, 1).await;
    assert_eq!(
        first.added.len(),
        2,
        "deeper down a volume's folder name is the person's, and is carried in",
    );

    // The same folder mapped as a top-level mapping's root: the Entry now
    // stands at that root's top, where the rule leaves the name out.
    fixture
        .index()
        .set_mapping(Mapping::new(
            Some(entry_path("albums")),
            Path::new(fixture.folder()).join("albums"),
        ))
        .await
        .expect("recording a mapping must succeed");

    let outcome = sync(fixture, 2).await;

    assert!(
        outcome.surfaced.is_empty(),
        "an Entry whose file the scan never looks for is not deleted locally: {:?}",
        outcome.surfaced,
    );
    assert!(outcome.commit.is_none(), "and nothing is committed");
    assert!(
        fixture
            .index()
            .entry_at(&entry_path(inner))
            .await
            .expect("asking the Index for a path must succeed")
            .is_some(),
        "the Entry stays until it is deleted",
    );
}
