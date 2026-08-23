use coffret_model::ContainerKind;

use crate::index_conformance::fixtures::{addition, container_id, observation, path, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// The Entry Path in NFC, and the same characters in NFD.
///
/// `é` is one code point in NFC and `e` followed by a combining acute in NFD.
/// They render identically and are different byte sequences, which is exactly
/// the pair a catalog must not fold together (spec: EP-1, EP-3).
const COMPOSED: &str = "albums/caf\u{e9}.jpg";
const DECOMPOSED: &str = "albums/cafe\u{301}.jpg";

/// Two Entry Paths differing only in case are two paths.
///
/// Equality is exact equality of the canonical bytes and is case-sensitive, so
/// a catalog that compared case-insensitively would answer one path with the
/// other's Entry — and would lose one of the user's files at the commit, where
/// the same comparison decides uniqueness (spec: EP-3, EP-5).
pub async fn case_distinguishes_two_entry_paths(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/Photo.jpg", "albums/photo.jpg"],
            )],
            vec![],
        ))
        .await
        .expect("two paths differing in case are two Entries");

    let upper = index
        .entry_at(&path("albums/Photo.jpg"))
        .await
        .expect("looking a path up must succeed")
        .expect("the upper-case path holds an Entry");
    let lower = index
        .entry_at(&path("albums/photo.jpg"))
        .await
        .expect("looking a path up must succeed")
        .expect("the lower-case path holds an Entry");

    assert_ne!(
        upper.extent(),
        lower.extent(),
        "the two paths answer with different Entries"
    );
    assert_eq!(
        index
            .entries_under(Some(&path("albums")))
            .await
            .expect("reading a subtree must succeed")
            .len(),
        2
    );
}

/// Two Entry Paths differing only in normalization form are two paths.
///
/// NFC is the canonical form an Entry Path is put into before it ever reaches
/// the catalog (spec: EP-1); the catalog itself compares the bytes it is given
/// and merges nothing, so a decomposed spelling that reached it is a second
/// path rather than a second name for the first.
pub async fn normalization_form_distinguishes_two_entry_paths(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::Pack, &[COMPOSED, DECOMPOSED])],
            vec![],
        ))
        .await
        .expect("two spellings of one name are two Entries to a byte comparison");

    let composed = index
        .entry_at(&path(COMPOSED))
        .await
        .expect("looking a path up must succeed")
        .expect("the composed path holds an Entry");
    let decomposed = index
        .entry_at(&path(DECOMPOSED))
        .await
        .expect("looking a path up must succeed")
        .expect("the decomposed path holds an Entry");

    assert_ne!(
        composed.extent(),
        decomposed.extent(),
        "the two spellings answer with different Entries"
    );
}

/// A prefix covers a subtree, and stops at the separator.
///
/// `/` is the only logical separator, so `books` covers the Entry at `books`
/// and everything under `books/` — and not `books-annex/…`, which sorts between
/// the two and would be swept in by a comparison on the raw prefix
/// (spec: EP-2, EP-3, EP-9).
pub async fn a_prefix_covers_a_subtree_and_stops_at_the_separator(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &[
                    "books",
                    "books-annex/page-001.png",
                    "books/page-001.png",
                    "books/some-novel/page-042.png",
                    "zebra.txt",
                ],
            )],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    let under = index
        .entries_under(Some(&path("books")))
        .await
        .expect("reading a subtree must succeed");
    let paths: Vec<&str> = under.iter().map(|entry| entry.path().as_str()).collect();
    assert_eq!(
        paths,
        [
            "books",
            "books/page-001.png",
            "books/some-novel/page-042.png"
        ]
    );

    let all = index
        .entries_under(None)
        .await
        .expect("reading the whole Library must succeed");
    assert_eq!(all.len(), 5, "the Library root covers everything");
}

/// The Containers under a prefix are reported once each.
///
/// Packs built by different `freeze` invocations may overlap and interleave in
/// path order, so one Container can hold several Entries under one prefix and
/// one prefix can span many Containers (spec: PK-8). What a caller wants is the
/// set to fetch, and fetching one Container twice is wasted bandwidth
/// (spec: PK-16).
pub async fn the_containers_under_a_prefix_are_reported_once(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![
                addition(
                    1,
                    ContainerKind::Pack,
                    &["albums/a.jpg", "albums/b.jpg", "books/page-001.png"],
                ),
                addition(2, ContainerKind::Pack, &["albums/c.jpg"]),
                addition(3, ContainerKind::OneFile, &["notes.txt"]),
            ],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");

    let under: Vec<_> = index
        .containers_under(Some(&path("albums")))
        .await
        .expect("reading a subtree's Containers must succeed")
        .into_iter()
        .map(|container| container.id)
        .collect();
    assert_eq!(under, [container_id(1), container_id(2)]);

    let all: Vec<_> = index
        .containers_under(None)
        .await
        .expect("reading the Library's Containers must succeed")
        .into_iter()
        .map(|container| container.id)
        .collect();
    assert_eq!(all, [container_id(1), container_id(2), container_id(3)]);
}

/// What this device has under a prefix is what it materialized there, not what
/// the Library holds there (spec: EP-9, EP-10).
pub async fn a_prefix_reports_only_what_this_device_materialized(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(
                1,
                ContainerKind::Pack,
                &["albums/a.jpg", "albums/b.jpg", "books/page-001.png"],
            )],
            vec![],
        ))
        .await
        .expect("replaying a record must succeed");
    index
        .mark_present(observation("albums/a.jpg", 100))
        .await
        .expect("recording a materialized file must succeed");
    index
        .mark_present(observation("books/page-001.png", 102))
        .await
        .expect("recording a materialized file must succeed");

    let present = index
        .present_under(Some(&path("albums")))
        .await
        .expect("reading what this device has must succeed");
    let paths: Vec<&str> = present
        .iter()
        .map(|local| local.observation.path.as_str())
        .collect();
    assert_eq!(
        paths,
        ["albums/a.jpg"],
        "a mapped subtree the device holds only part of is a partial subtree"
    );
}
