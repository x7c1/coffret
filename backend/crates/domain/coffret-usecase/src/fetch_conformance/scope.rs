use coffret_model::EntryPath;

use crate::fetch::fetch_folders;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{exists, keys, map, read, request, sync_source, write};

/// A prefix-narrowed fetch places one subtree and touches nothing else.
///
/// The narrowing is an intersection with the mappings and not a substitute for
/// them: a mapping is what makes a local path exist for an Entry Path at all
/// (spec: EP-9), so the run covers exactly the part of the mapped subtree the
/// prefix names. Which is also the state EP-10 describes as ordinary rather than
/// broken — a device that maps `albums/` and has fetched only part of it holds a
/// partial subtree, and the rest does not count as deleted.
///
/// The Containers are the evidence: one per file here, so a run that fetched
/// beyond its prefix would say so in the count.
pub async fn a_prefix_narrows_the_fetch_to_one_subtree(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    let wanted = b"a photo from the spring of 2026".as_slice();
    write(fixture.source_folder(), "albums/2026/spring.jpg", wanted).await;
    write(
        fixture.source_folder(),
        "albums/2025/winter.jpg",
        b"an older photo",
    )
    .await;
    write(fixture.source_folder(), "books/page-1.png", b"a page").await;
    sync_source(fixture, &keys, 1).await;

    let outcome = fetch_folders(
        request(fixture.store(), fixture.target(), &keys, 2).under(EntryPath::nfc("albums/2026")),
    )
    .await
    .unwrap_or_else(|error| panic!("a narrowed fetch must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        vec![EntryPath::nfc("albums/2026/spring.jpg")],
        "only the subtree the prefix names",
    );
    assert_eq!(
        outcome.containers.len(),
        1,
        "and only its Container was fetched (spec: PK-16)",
    );
    assert_eq!(outcome.skipped, 0);
    assert!(
        outcome.surfaced.is_empty(),
        "an Entry outside the prefix was never selected, so it is not a finding",
    );

    assert_eq!(
        read(&fixture.target_folder().join("albums/2026/spring.jpg")).await,
        wanted,
    );
    for outside in ["albums/2025/winter.jpg", "books/page-1.png"] {
        assert!(
            !exists(&fixture.target_folder().join(outside)).await,
            "{outside} is outside the prefix and was left alone",
        );
        assert!(
            fixture
                .target()
                .local_entry_at(&EntryPath::nfc(outside))
                .await
                .expect("asking the target catalog for a local row must succeed")
                .is_none(),
            "a run that placed nothing at {outside} invents no claim to have placed it",
        );
    }
}

/// A mapped prefix decides where a fetched file lands (spec: EP-9).
///
/// The same Library, arranged differently on the two devices: the source maps its
/// folder at the Library root and the target maps its own at `albums/`, so the
/// file the source wrote at `albums/2026/spring.jpg` arrives at
/// `2026/spring.jpg` under the target's folder. The mappings are device state and
/// are never uploaded, which is exactly what makes that possible (spec: CK-7).
pub async fn a_mapped_prefix_decides_where_a_fetched_file_lands(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), Some("albums"), fixture.target_folder()).await;

    let content = b"a photo".as_slice();
    write(fixture.source_folder(), "albums/2026/spring.jpg", content).await;
    write(fixture.source_folder(), "books/page-1.png", b"a page").await;
    sync_source(fixture, &keys, 1).await;

    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2))
        .await
        .unwrap_or_else(|error| panic!("a fetch into a mapped subtree must succeed: {error}"));

    assert_eq!(
        outcome.fetched,
        vec![EntryPath::nfc("albums/2026/spring.jpg")],
        "the mapping covers `albums/` and nothing else, so nothing else was selected",
    );
    assert_eq!(
        read(&fixture.target_folder().join("2026/spring.jpg")).await,
        content,
        "the mapping's prefix is stripped off the local path it gives the Entry",
    );
    assert!(
        !exists(&fixture.target_folder().join("albums/2026/spring.jpg")).await,
        "the Entry Path is not also spelled out below the mapped root",
    );
}
