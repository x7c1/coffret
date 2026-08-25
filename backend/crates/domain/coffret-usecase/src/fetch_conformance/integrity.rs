use coffret_model::{EntryPath, Mtime};

use crate::fetch::{fetch_folders, FetchError};
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    container_handle, entry_at, exists, keys, map, plant, read, request, scratch_left, sync_source,
    write, Planted, OLDER,
};
use crate::fetch_conformance::mangling_store::ManglingStore;

/// An object at a Container's name that is not a Container stops the run, and
/// places nothing.
///
/// The bytes are what the record says they are — the record's ciphertext hash is
/// the hash of exactly what is stored — so the fetch gets past its first check
/// and presents them to the key the committed Keyring holds. What refuses them is
/// the format layer, and that refusal is the whole of the guarantee: authentication
/// happens per chunk, before a byte reaches a caller's buffer (spec: FM-5, FM-8),
/// so there was never a moment at which unverified content could have been
/// written.
///
/// What matters as much is what is *not* on disk afterwards: no file at the
/// target path, and no temporary one either (spec: EP-11).
pub async fn a_container_that_does_not_decode_is_refused(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.target(), None, fixture.target_folder()).await;

    plant(
        fixture.store(),
        fixture.source(),
        &keys,
        Planted {
            path: "a.jpg",
            content: b"what the record says the Entry holds",
            mtime: Mtime::from_unix_seconds(OLDER),
            real: false,
            actual_content: None,
        },
    )
    .await;

    let result = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2)).await;

    let Err(FetchError::Format(error)) = result else {
        panic!("expected an object that is not a Container to be refused, got {result:?}");
    };
    // The message is the format layer's; what this case asserts is that the
    // refusal came from there and not from a check the fetch made up.
    assert!(!error.to_string().is_empty());

    assert!(
        !exists(&fixture.target_folder().join("a.jpg")).await,
        "nothing unverified reaches a target path (spec: EP-11)",
    );
    assert_eq!(
        scratch_left(fixture.target_folder()).await,
        0,
        "and the temporary file the run may have made is gone",
    );
    assert!(
        fixture
            .target()
            .local_entry_at(&EntryPath::new("a.jpg"))
            .await
            .expect("asking the target catalog for a local row must succeed")
            .is_none(),
        "a run that placed nothing claims nothing (spec: EP-10)",
    );
}

/// A Container whose ciphertext is not the ciphertext the record hashed stops the
/// run, and places nothing.
///
/// The damage happens in transit, which is the only place it can be tested from:
/// bytes written wrongly into the bucket beforehand would make this a question
/// about what the Library holds rather than about what arrived. So the store hands
/// back one altered byte, and the run compares what it received against the hash
/// its Journal record carries and refuses the difference before a key is unwrapped
/// at all (spec: FM-15, CP-11).
///
/// The second file is there so that the refusal is a verdict about one Container
/// and not about the run: whichever of the two the walk reaches first, a later run
/// against an honest store finishes the folder.
pub async fn a_container_whose_ciphertext_differs_is_refused(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.source(), None, fixture.source_folder()).await;
    map(fixture.target(), None, fixture.target_folder()).await;

    write(
        fixture.source_folder(),
        "a.jpg",
        b"the file that arrives whole",
    )
    .await;
    write(fixture.source_folder(), "b.jpg", b"the file that does not").await;
    sync_source(fixture, &keys, 1).await;

    // Asked of the device that committed it: the fetching device has not caught
    // up yet, which is the very thing the run is about to do.
    let damaged = entry_at(fixture.source(), "b.jpg").await.container_id;
    let mangling = ManglingStore::around(
        fixture.store(),
        container_handle(fixture.store(), damaged).await,
    );

    let result = fetch_folders(request(&mangling, fixture.target(), &keys, 2)).await;

    let Err(FetchError::CiphertextMismatch {
        container_id,
        expected,
        actual,
    }) = result
    else {
        panic!("expected a damaged Container to be refused, got {result:?}");
    };
    assert_eq!(container_id, damaged);
    assert_ne!(expected, actual);

    assert!(
        !exists(&fixture.target_folder().join("b.jpg")).await,
        "nothing unverified reaches a target path (spec: EP-11)",
    );
    assert_eq!(scratch_left(fixture.target_folder()).await, 0);

    // And a later run, against a store that answers honestly, converges —
    // whatever the refused run had already placed stays placed, since Containers
    // are walked in Container ID order rather than in the order a case wrote the
    // files.
    let outcome = fetch_folders(request(fixture.store(), fixture.target(), &keys, 3))
        .await
        .expect("a run against an honest store must succeed");
    assert!(outcome.fetched.contains(&EntryPath::new("b.jpg")));
    assert_eq!(
        outcome.fetched.len() + outcome.skipped,
        2,
        "every Entry is either placed by this run or was placed by the last",
    );
    assert!(outcome.surfaced.is_empty());
    assert_eq!(
        read(&fixture.target_folder().join("a.jpg")).await,
        b"the file that arrives whole",
    );
    assert_eq!(
        read(&fixture.target_folder().join("b.jpg")).await,
        b"the file that does not",
    );
}

/// A Container that holds content the catalog does not name stops the run.
///
/// The object is a real Container, sealed under the key the committed Keyring
/// maps it to, and it authenticates: every chunk verifies against the entry table
/// inside it. What it does not agree with is the entry table the *Journal record*
/// carried, which is what the Index answers from (spec: CP-11). Authenticity says
/// the bytes are a coffret object; the hash comparison says they are the committed
/// content this catalog stands for, and only the second one catches this
/// (spec: FM-9, EP-11).
pub async fn a_container_whose_content_is_not_what_the_catalog_names_is_refused(
    fixture: &FetchUnderTest,
) {
    let keys = keys();
    map(fixture.target(), None, fixture.target_folder()).await;

    let planted = plant(
        fixture.store(),
        fixture.source(),
        &keys,
        Planted {
            path: "a.jpg",
            content: b"the content the record's entry table describes",
            mtime: Mtime::from_unix_seconds(OLDER),
            real: true,
            actual_content: Some(b"the content the object really holds"),
        },
    )
    .await;

    let result = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2)).await;

    let Err(FetchError::ContentMismatch { container_id, path }) = result else {
        panic!("expected content the catalog does not name to be refused, got {result:?}");
    };
    assert_eq!(container_id, planted);
    assert_eq!(path, EntryPath::new("a.jpg"));

    assert!(
        !exists(&fixture.target_folder().join("a.jpg")).await,
        "an authentic Container is still not the content the catalog names (spec: EP-11)",
    );
    assert_eq!(scratch_left(fixture.target_folder()).await, 0);
}
