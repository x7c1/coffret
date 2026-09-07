use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::spool_conformance::spool_under_test::SpoolUnderTest;
use crate::spool_conformance::CIPHERTEXT;

/// A spool under a directory nobody made is refused as a creation.
///
/// A run prepares the spool directory once and creates every file under it, so
/// the failure this stands for is a directory that went away — or one a caller
/// never asked for. What matters is which operation it comes back as: a caller
/// telling somebody what to go and fix reads
/// [`LocalOperation`], never a message.
pub async fn a_spool_under_an_unprepared_directory_is_refused(fixture: &SpoolUnderTest) {
    let spool = fixture.spool();
    // Under the fixture's own directory, so that nothing outside it is touched
    // even by a spool that were somehow created.
    let path = fixture
        .dir()
        .join("never-prepared")
        .join("a-container.spool");

    let result = spool.create(&path).await;
    let Err(LocalIoError { operation, .. }) = result else {
        panic!("a spool under a directory nobody made must be refused");
    };
    assert!(
        matches!(operation, LocalOperation::Creating),
        "the refusal is about creating the file, and says so: {operation}",
    );
}

/// The directory an earlier run prepared is prepared again by the next one.
///
/// Every sync and every freeze prepares the spool directory before it spools
/// anything, so every run of a device after the first meets the directory the
/// one before it made. Preparing is `mkdir -p` and not `mkdir`: a spool that
/// refused the second one would fail every run but the first, and nothing above
/// asks whether the directory is there first — no flow lists the spool, because
/// the pending rows are its only index (spec: OC-2).
pub async fn a_directory_an_earlier_run_prepared_is_prepared_again(fixture: &SpoolUnderTest) {
    let spool = fixture.spool();
    spool
        .prepare_dir(fixture.dir())
        .await
        .expect("preparing the spool directory must succeed");
    spool
        .prepare_dir(fixture.dir())
        .await
        .expect("preparing a directory that is already there must succeed");

    // And what the second run has it for still works, which is the whole of why
    // it prepared it.
    let path = fixture.dir().join("a-container.spool");
    let mut writer = spool
        .create(&path)
        .await
        .expect("creating under a directory prepared twice must succeed");
    writer
        .write(CIPHERTEXT)
        .await
        .expect("writing must succeed");
    writer.finish().await.expect("flushing must succeed");
    spool
        .discard(&path)
        .await
        .expect("removing a spool must succeed");
}
