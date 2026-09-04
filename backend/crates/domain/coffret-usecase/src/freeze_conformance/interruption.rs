use coffret_model::ContainerId;

use crate::conformance_library::Library;
use crate::device_state::SpoolState;
use crate::freeze::{freeze_folder, FreezeError, LibraryKeys};
use crate::freeze_conformance::fixtures::{
    filler, keys, map, pending, request, spooled, sync_source, write, WatchingIndex, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::index_error::IndexError;
use crate::sync::Reconciled;

/// Writes the folder these cases freeze, and answers with how many files it has.
///
/// Sizes on both sides of the target, so the run is cut into several Packs —
/// which is what makes "one row per Pack" a claim about more than the first one.
async fn files(fixture: &FreezeUnderTest) -> usize {
    let files: Vec<(String, Vec<u8>)> = (0..12)
        .map(|index| {
            (
                format!("albums/2026/{index:03}.jpg"),
                filler(45 + (index * 37) % 170, 0x70 + index as u8),
            )
        })
        .collect();
    for (relative, content) in &files {
        write(fixture.source_folder(), relative, content).await;
    }
    files.len()
}

/// Every Pack is named by a row before its first byte reaches the spool
/// (spec: OC-2).
///
/// It matters more for a Pack than for one file. A Pack is around a gigabyte and
/// an oversized singleton is whatever one indivisible Entry weighs (spec: PK-3,
/// PK-5), so the file a dead run leaves behind can be enormous — and nothing in
/// the flow ever lists the spool directory, so the row is the only handle on it
/// there will ever be.
///
/// So the row goes first, and the catalog the run drives asserts it as each row
/// arrives: the path it names must not be there yet, and it may carry no object
/// handle. One such row per Pack the run reports is what says the ordering held
/// for all of them and not merely for the first.
pub async fn a_row_precedes_the_first_byte_of_a_pack_spool(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let count = files(fixture).await;

    let watching = WatchingIndex::around(index);
    let outcome = freeze_folder(request(store, &watching, &keys, fixture.spool(), TARGET, 1))
        .await
        .expect("a watched freeze must succeed");

    assert!(
        outcome.packs.len() > 1,
        "the target is small enough that the folder is cut several times, got {} Pack(s)",
        outcome.packs.len(),
    );
    assert_eq!(outcome.frozen_entries(), count);
    assert_eq!(
        watching.spooling_rows(),
        outcome.packs.len(),
        "every Pack the run spooled was announced before its file existed",
    );
    assert!(
        pending(index).await.is_empty(),
        "a committed batch leaves no rows behind (spec: OC-2)",
    );
}

/// A Pack spool the run never finished is disposed of, along with its row.
///
/// The run stops between creating a spool file and marking it `Spooled`, so
/// what is at the path is ciphertext no row calls whole — here a Pack the run did
/// finish writing and never got to mark, elsewhere half of one or no bytes at
/// all — and nothing on Storage or in the Library will ever refer to it either
/// way. The row over it says exactly that much.
///
/// A freeze does not settle rows itself — what an interrupted one leaves is the
/// sync flow's to settle, whatever wrote it — so the next `sync_folders` is what
/// reclaims both (spec: OC-2, OC-3). It needs no head to do it: nothing that was
/// never uploaded can be current.
pub async fn an_unfinished_pack_spool_is_disposed_with_its_row(fixture: &FreezeUnderTest) {
    let index = fixture.source();
    let keys = keys();
    let abandoned = interrupted_spool(fixture, &keys).await;

    let outcome = sync_source(fixture, &keys, 2).await;

    assert_eq!(
        outcome.reconciled,
        vec![Reconciled::Disposed {
            container_id: abandoned,
            // It never left the device, so there was nothing on Storage to
            // remove.
            trashed: false,
        }],
        "an unfinished Pack spool is this device's own to reclaim (spec: OC-2)",
    );
    assert!(
        !outcome.added.contains(&abandoned),
        "the abandoned Pack is not something a later run resumes (spec: KD-2)",
    );

    let commit = outcome
        .commit
        .expect("the files the freeze never committed are worth a commit");
    assert!(
        !commit
            .record
            .additions()
            .iter()
            .any(|addition| addition.container().id == abandoned),
        "the abandoned Pack is not what the batch committed",
    );

    assert!(
        pending(index).await.is_empty(),
        "the provenance goes with what it was provenance for (spec: OC-2)",
    );
    assert_eq!(
        spooled(fixture.spool()).await,
        0,
        "neither the abandoned Pack nor the committed Containers are still on disk",
    );
}

/// A Spooling Pack row is never uploaded and never committed.
///
/// The claim the shape of the flow makes rather than the reconcile: a
/// `SpooledContainer` exists only for a spool that was finished, and the upload,
/// the verification, and the commit all act on that list alone. So the spool the
/// interrupted run left never reached Storage, and no Journal record ever names
/// it — before the reclamation as much as after it.
pub async fn a_spooling_pack_row_is_never_uploaded_or_committed(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    let abandoned = interrupted_spool(fixture, &keys).await;

    assert!(
        !Library::read(store).await.holds_container(abandoned),
        "the run failed before the list an upload walks could hold this Pack",
    );

    let outcome = sync_source(fixture, &keys, 2).await;

    let commit = outcome
        .commit
        .expect("the files the freeze never committed are worth a commit");
    assert!(
        !commit
            .record
            .additions()
            .iter()
            .any(|addition| addition.container().id == abandoned),
        "no record adds the abandoned Pack",
    );
    assert!(
        !commit.record.removals().contains(&abandoned),
        "a Container no record ever added is not one a record removes",
    );
    assert!(
        !index
            .containers_under(None)
            .await
            .expect("asking the Index for the current Containers must succeed")
            .iter()
            .any(|container| container.id == abandoned),
        "the catalog replayed every record there is, and none of them names it",
    );
    assert!(
        !Library::read(store).await.holds_container(abandoned),
        "and nothing put it on Storage on the way past",
    );
}

/// Leaves what a freeze interrupted between creating a spool file and marking it
/// `Spooled` leaves: one file on disk that nothing calls a whole Pack, and one
/// row saying exactly that about it.
///
/// Driven rather than planted, because what is being set up is an ordering inside
/// one call. The catalog the run writes into is the fixture's own, wrapped only to
/// refuse that one write, so what the case goes on to inspect really is what the
/// interrupted run left behind.
///
/// Answers with the Container the abandoned spool would have been.
async fn interrupted_spool(fixture: &FreezeUnderTest, keys: &LibraryKeys) -> ContainerId {
    let index = fixture.source();
    map(index, None, fixture.source_folder()).await;
    files(fixture).await;

    let watching = WatchingIndex::refusing_to_mark_spooled(index);
    let result = freeze_folder(request(
        fixture.store(),
        &watching,
        keys,
        fixture.spool(),
        TARGET,
        1,
    ))
    .await;
    let Err(FreezeError::Index(IndexError::Backend { .. })) = &result else {
        panic!("a refused marking must fail the run that spooled, got {result:?}");
    };
    assert_eq!(
        watching.spooling_rows(),
        1,
        "the run stopped at the first Pack, having announced it first",
    );

    let rows = pending(index).await;
    assert_eq!(rows.len(), 1, "one Pack was announced, so one row is open");
    assert_eq!(
        rows[0].state,
        SpoolState::Spooling,
        "the run never got to say the Pack was whole",
    );
    assert!(
        rows[0].object_ref.is_none(),
        "an unfinished Pack spool is never uploaded",
    );
    assert_eq!(
        spooled(fixture.spool()).await,
        1,
        "the ciphertext the run did write is still on disk, and the row names it",
    );
    rows[0].container_id
}
