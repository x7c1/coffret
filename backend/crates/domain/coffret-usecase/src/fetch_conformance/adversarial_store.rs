use coffret_format::Header;
use coffret_model::Mtime;

use crate::entry_paths::entry_path;
use crate::fetch::{fetch_entry, fetch_folders, FetchError};
use crate::fetch_conformance::counting_store::CountingStore;
use crate::fetch_conformance::fetch_under_test::FetchUnderTest;
use crate::fetch_conformance::fixtures::{
    container_handle, entry_request, exists, keys, map, plant, request, scratch_left, Planted,
    OLDER,
};

/// A Container whose header declares a meta section that could not exist is
/// refused, and nothing is allocated for the declaration.
///
/// The four bytes of a Container header that hold the meta section length are
/// plaintext and unauthenticated (spec: FM-2), and everything a reader does next
/// is sized by them. Storage is outside the trust boundary — a provider bug,
/// somebody else with write access to the account, or a proxy in between can all
/// put a number there — so a reader that took the field at its word would spend
/// four gigabytes of memory before the AEAD told it the object was never
/// authentic.
///
/// The object here is a real Container with those four bytes overwritten, and
/// the record hashes exactly what is stored, so the run gets past its ciphertext
/// check and reaches the decode — which is where the declaration is answered,
/// having read the object's front and nothing more.
pub async fn a_container_declaring_an_impossible_meta_section_is_refused(fixture: &FetchUnderTest) {
    let keys = keys();
    map(fixture.target(), None, fixture.target_folder()).await;

    plant(
        fixture.store(),
        fixture.source(),
        &keys,
        Planted {
            path: "a.jpg",
            content: b"the content the record's entry table describes",
            mtime: Mtime::from_unix_seconds(OLDER),
            real: true,
            actual_content: None,
            // The largest allocation four edited bytes can ask for.
            meta_len: Some(u32::MAX),
        },
    )
    .await;

    let result = fetch_folders(request(fixture.store(), fixture.target(), &keys, 2)).await;

    let Err(FetchError::Format(error)) = result else {
        panic!("expected an impossible meta section to be refused, got {result:?}");
    };
    // Which refusal it is, is the point: the declared length was held against
    // what a meta section may be, rather than acted on and regretted.
    assert!(
        matches!(
            error,
            coffret_format::Error::MetaSectionTooLong { declared, limit }
                if declared == u64::from(u32::MAX)
                    && limit == u64::from(Header::MAX_META_LEN)
        ),
        "expected the declaration itself to be refused, got {error:?}",
    );

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
            .local_entry_at(&entry_path("a.jpg"))
            .await
            .expect("asking the target catalog for a local row must succeed")
            .is_none(),
        "a run that placed nothing claims nothing (spec: EP-10)",
    );
}

/// The same Container declined by a partial fetch, before it asks Storage for
/// the section it declares.
///
/// This is the path where a lying length costs the most, because the reader does
/// not merely size a buffer by it — it aims a *range request* at it, and would
/// ask a provider for four gigabytes of an object that is a few hundred bytes
/// long (spec: FM-2, PK-16). So the case checks what was asked of Storage: one
/// read of the Container, carrying the header's own 32-byte range, and no second
/// read at all.
pub async fn a_partial_fetch_of_an_impossible_meta_section_asks_for_nothing_more(
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
            actual_content: None,
            meta_len: Some(u32::MAX),
        },
    )
    .await;
    let object = container_handle(fixture.store(), planted).await;

    let counting = CountingStore::around(fixture.store());
    let result = fetch_entry(entry_request(
        &counting,
        fixture.target(),
        &keys,
        "a.jpg",
        2,
    ))
    .await;

    let Err(FetchError::Format(error)) = result else {
        panic!("expected an impossible meta section to be refused, got {result:?}");
    };
    assert!(
        matches!(
            error,
            coffret_format::Error::MetaSectionTooLong { declared, limit }
                if declared == u64::from(u32::MAX)
                    && limit == u64::from(Header::MAX_META_LEN)
        ),
        "expected the declaration itself to be refused, got {error:?}",
    );

    let ranges = counting.ranges_of(&object);
    assert_eq!(
        ranges,
        vec![Some(0..Header::LEN as u64)],
        "the header was read, the declaration was refused, and nothing else was asked for",
    );

    assert!(
        !exists(&fixture.target_folder().join("a.jpg")).await,
        "nothing unverified reaches a target path (spec: EP-11)",
    );
    assert_eq!(scratch_left(fixture.target_folder()).await, 0);
}
