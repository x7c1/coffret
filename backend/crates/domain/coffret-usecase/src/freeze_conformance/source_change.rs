use crate::device_state::SpoolState;
use crate::entry_paths::entry_path;
use crate::freeze::{freeze_folder, FreezeError, SourceChange};
use crate::freeze_conformance::fixtures::{
    filler, keys, map, pending, request, spooled, write, TARGET,
};
use crate::freeze_conformance::freeze_under_test::FreezeUnderTest;
use crate::freeze_conformance::truncating_index::TruncatingIndex;

/// What the scan measures the file at.
const SURVEYED: usize = 180;

/// What is left of it by the time the Pack reads it.
const KEPT: u64 = 100;

/// A file that stops being the file the scan measured stops the Pack, and the
/// failure says what moved.
///
/// A Pack's entry table is written before its content, because that is what lets
/// the content stream (spec: FM-2, FM-5, FM-9), so a file whose length moved in
/// between would land inside a Container whose table does not describe it. The
/// run stops instead, and the object is never written — what is on disk is a
/// spool this device's own pending row accounts for (spec: OC-2), and the file
/// is simply eligible again next time.
///
/// The stop alone would leave a person with "a local file changed" and the path,
/// which is the same sentence whether a file was being written while the run
/// read it or something rewrote history under it. So the case asserts the cause
/// as well as the verdict: the length the scan recorded and the length that
/// actually arrived both come back, which is what makes the first reading
/// available at all.
///
/// The change is made from inside the run rather than planted beforehand,
/// because the window it has to land in is inside one call — see
/// [`TruncatingIndex`].
pub async fn a_file_that_shrinks_under_the_run_stops_its_pack(fixture: &FreezeUnderTest) {
    let store = fixture.store();
    let index = fixture.source();
    let keys = keys();
    map(index, None, fixture.source_folder()).await;

    let local = write(
        fixture.source_folder(),
        "albums/a.jpg",
        &filler(SURVEYED, 0x40),
    )
    .await;

    let truncating = TruncatingIndex::shortening(index, &local, KEPT);
    let result = freeze_folder(request(
        store,
        &truncating,
        &keys,
        fixture.spool(),
        TARGET,
        1,
    ))
    .await;

    let Err(FreezeError::SourceChanged { path, cause }) = result else {
        panic!("a file that moved under the run must stop it, got {result:?}");
    };
    assert_eq!(
        path,
        entry_path("albums/a.jpg"),
        "the failure names the file that moved",
    );
    let SourceChange::LengthMoved { expected, actual } = cause else {
        panic!("expected the length that moved to come back, got {cause:?}");
    };
    assert_eq!(
        expected, SURVEYED as u64,
        "the cause carries the length the entry table records",
    );
    assert_eq!(actual, KEPT, "and the length that actually arrived");

    assert!(
        index
            .containers_under(None)
            .await
            .expect("asking the catalog for the current Containers must succeed")
            .is_empty(),
        "a Pack whose table would lie about its content is never committed",
    );
    let rows = pending(index).await;
    assert_eq!(rows.len(), 1, "the abandoned Pack is named by its own row");
    assert_eq!(
        rows[0].state,
        SpoolState::Spooling,
        "the run never got to say the Pack was whole (spec: OC-2)",
    );
    assert_eq!(
        spooled(fixture.spool()).await,
        1,
        "what the run did write is on disk, and the row names it",
    );
}
