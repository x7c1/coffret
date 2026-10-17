use crate::entry_paths::entry_path;
use crate::index_error::IndexError;
use crate::local_operation::LocalOperation;
use crate::relocate::RelocationRefused;
use crate::sync::SyncError;
use crate::sync_conformance::refusing_index::RefusingIndex;
use crate::sync_conformance::sync_under_test::SyncUnderTest;

use super::{
    at, carry_in, content, refused, relocated, rename, row, sync, sync_through, uploaded_nothing,
};

/// A relocation the disk refuses before its rename is made leaves the file and
/// its record as they were, and the next run makes it (spec: EP-10).
pub async fn a_relocation_interrupted_before_its_rename_is_made_by_the_next_run(
    fixture: &SyncUnderTest,
) {
    carry_in(fixture, &[("a.jpg", b"the file's bytes")]).await;
    rename(fixture, &[("a.jpg", "b.jpg")]).await;
    fixture.fs().fail_on(LocalOperation::Relocating, 1);

    let outcome = sync(fixture, 2).await;

    uploaded_nothing(&outcome);
    let (_, _, reason) = refused(&outcome);
    assert!(matches!(reason, RelocationRefused::Disk(_)), "{reason:?}");
    assert!(fixture.fs().holds(&at(fixture, "a.jpg")));
    assert!(!fixture.fs().holds(&at(fixture, "b.jpg")));
    assert_eq!(
        row(fixture, "a.jpg")
            .await
            .expect("the record stays")
            .relocate_to,
        Some(entry_path("b.jpg"))
    );

    let next = sync(fixture, 3).await;
    uploaded_nothing(&next);
    assert_eq!(relocated(&next), [("a.jpg".to_owned(), "b.jpg".to_owned())]);
}

/// A run interrupted after the rename and before the record follows leaves
/// the file at the new path and the record at the old one. The next run finds
/// the file where its Entry stands, as the record describes it, and re-keys
/// the record: nothing is relocated twice, uploaded, overwritten, or lost
/// (spec: EP-10).
pub async fn a_relocation_interrupted_after_its_rename_is_finished_by_the_next_run(
    fixture: &SyncUnderTest,
) {
    carry_in(fixture, &[("inbox/a.jpg", b"the file's bytes")]).await;
    rename(fixture, &[("inbox/a.jpg", "albums/a.jpg")]).await;

    let interrupted = sync_through(
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
    assert_eq!(
        content(fixture, "albums/a.jpg").as_deref(),
        Some(&b"the file's bytes"[..]),
        "the file has been relocated"
    );
    assert_eq!(
        row(fixture, "inbox/a.jpg")
            .await
            .expect("and its record has not")
            .relocate_to,
        Some(entry_path("albums/a.jpg"))
    );

    let next = sync(fixture, 3).await;

    uploaded_nothing(&next);
    assert_eq!(
        relocated(&next),
        [("inbox/a.jpg".to_owned(), "albums/a.jpg".to_owned())]
    );
    assert!(row(fixture, "inbox/a.jpg").await.is_none());
    assert_eq!(
        row(fixture, "albums/a.jpg")
            .await
            .expect("the record follows the file")
            .relocate_to,
        None
    );
    assert_eq!(
        content(fixture, "albums/a.jpg").as_deref(),
        Some(&b"the file's bytes"[..])
    );
    assert!(next.surfaced.is_empty(), "{:?}", next.surfaced);
    assert!(fixture.fs().discarded().is_empty());
    assert_eq!(sync(fixture, 4).await.unchanged, 1);
}

/// Two files exchanged on disk whose records the run did not get to swap: the
/// next run finds each where the other's record says, as that record
/// describes it, and swaps the records rather than the files again
/// (spec: EP-10, FM-15).
pub async fn an_exchange_interrupted_before_its_records_is_finished_by_the_next_run(
    fixture: &SyncUnderTest,
) {
    carry_in(fixture, &[("a.jpg", b"first"), ("b.jpg", b"the second")]).await;
    rename(fixture, &[("a.jpg", "b.jpg"), ("b.jpg", "a.jpg")]).await;

    let interrupted = sync_through(
        fixture,
        &RefusingIndex::refusing_relocations(fixture.index()),
        2,
    )
    .await;
    assert!(interrupted.is_err(), "the records could not follow");
    assert_eq!(content(fixture, "b.jpg").as_deref(), Some(&b"first"[..]));
    assert_eq!(
        content(fixture, "a.jpg").as_deref(),
        Some(&b"the second"[..])
    );

    let next = sync(fixture, 3).await;

    uploaded_nothing(&next);
    assert_eq!(
        relocated(&next),
        [
            ("a.jpg".to_owned(), "b.jpg".to_owned()),
            ("b.jpg".to_owned(), "a.jpg".to_owned()),
        ]
    );
    assert_eq!(content(fixture, "b.jpg").as_deref(), Some(&b"first"[..]));
    assert_eq!(
        content(fixture, "a.jpg").as_deref(),
        Some(&b"the second"[..])
    );
    for text in ["a.jpg", "b.jpg"] {
        assert_eq!(
            row(fixture, text)
                .await
                .expect("each keeps a record")
                .relocate_to,
            None
        );
    }
    assert_eq!(sync(fixture, 4).await.unchanged, 2);
}
