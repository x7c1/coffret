use coffret_model::ContainerKind;

use crate::index_conformance::fixtures::{addition, container_id, observation, path, record};
use crate::index_conformance::index_under_test::IndexUnderTest;

/// One Entry Path spelled with a half-width character, and the same name with
/// its full-width variant.
///
/// Both are already NFC: composing the two into one is what a *compatibility*
/// normalization does, and NFC is not one. So this is a pair that genuinely
/// reaches a catalog as two Entry Paths — a decomposed one no longer can, since
/// an [`EntryPath`](coffret_model::EntryPath) is NFC by construction — and the
/// two have to stay two (spec: EP-1, EP-3).
const HALF_WIDTH: &str = "albums/\u{ff71}.jpg";
const FULL_WIDTH: &str = "albums/\u{30a2}.jpg";

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

/// Two Entry Paths differing only in character width are two paths.
///
/// NFC is the canonical form an Entry Path is put into before it ever reaches
/// the catalog (spec: EP-1), and it merges neither case nor width variants nor
/// merely similar-looking characters. The catalog compares the bytes it is
/// given and folds nothing further, so a name and its width variant are two
/// Library positions rather than two spellings of one (spec: EP-3).
pub async fn width_variants_are_two_entry_paths(fixture: &IndexUnderTest) {
    let index = fixture.index();

    index
        .apply(record(
            0,
            vec![addition(1, ContainerKind::Pack, &[HALF_WIDTH, FULL_WIDTH])],
            vec![],
        ))
        .await
        .expect("two width variants are two Entries to a byte comparison");

    let half = index
        .entry_at(&path(HALF_WIDTH))
        .await
        .expect("looking a path up must succeed")
        .expect("the half-width path holds an Entry");
    let full = index
        .entry_at(&path(FULL_WIDTH))
        .await
        .expect("looking a path up must succeed")
        .expect("the full-width path holds an Entry");

    assert_ne!(
        half.extent(),
        full.extent(),
        "the two variants answer with different Entries"
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
