use tokio::io::AsyncReadExt;

use crate::local_io_error::LocalIoError;
use crate::local_operation::LocalOperation;
use crate::spool_conformance::spool_under_test::SpoolUnderTest;
use crate::spool_conformance::CIPHERTEXT;

/// The whole lifecycle of one spool, in the order a run performs it.
///
/// Prepare the directory, create the file, write it in two stretches — because a
/// Pack is written a buffer at a time and a suite that wrote once would not say
/// so — flush it, read it back, and remove it. What comes back has to be the
/// bytes that went in and all of them: a spool is what an upload sends, so a
/// reader that dropped or reordered anything would put a Container on Storage
/// that no device can open.
pub async fn a_spool_is_written_flushed_read_back_and_removed(fixture: &SpoolUnderTest) {
    let spool = fixture.spool();
    let path = fixture.dir().join("a-container.spool");
    spool
        .prepare_dir(fixture.dir())
        .await
        .expect("preparing the spool directory must succeed");

    let mut writer = spool
        .create(&path)
        .await
        .expect("creating a spool file under a prepared directory must succeed");
    let (front, back) = CIPHERTEXT.split_at(16);
    writer.write(front).await.expect("writing must succeed");
    writer.write(back).await.expect("writing must succeed");
    writer.finish().await.expect("flushing must succeed");

    let mut read = Vec::new();
    spool
        .open(&path)
        .await
        .expect("a finished spool must open")
        .read_to_end(&mut read)
        .await
        .expect("reading a spool back must succeed");
    assert_eq!(
        read, CIPHERTEXT,
        "a spool reads back as the bytes that were written to it",
    );

    spool
        .discard(&path)
        .await
        .expect("removing a spool must succeed");
    let Err(LocalIoError { operation, .. }) = spool.open(&path).await else {
        panic!("a discarded spool is gone, and opening it answers so");
    };
    assert!(
        matches!(operation, LocalOperation::Reading),
        "the refusal is about reading the file that is no longer there: {operation}",
    );
}

/// Creating a spool at a path that already holds one replaces it.
///
/// A run that was interrupted between writing a spool and marking it whole
/// leaves ciphertext no key opens, and the next run over that Container writes a
/// new one at the same path (spec: OC-2). What must not survive that is the old
/// content: a spool whose new bytes were appended to the old ones is a Container
/// no device can decode.
pub async fn a_created_spool_replaces_what_was_at_the_path(fixture: &SpoolUnderTest) {
    let spool = fixture.spool();
    let path = fixture.dir().join("a-container.spool");
    spool
        .prepare_dir(fixture.dir())
        .await
        .expect("preparing the spool directory must succeed");

    let mut writer = spool.create(&path).await.expect("creating must succeed");
    writer
        .write(b"what an interrupted run left behind")
        .await
        .expect("writing must succeed");
    writer.finish().await.expect("flushing must succeed");

    let mut writer = spool
        .create(&path)
        .await
        .expect("creating over an abandoned spool must succeed");
    writer
        .write(CIPHERTEXT)
        .await
        .expect("writing must succeed");
    writer.finish().await.expect("flushing must succeed");

    let mut read = Vec::new();
    spool
        .open(&path)
        .await
        .expect("the spool must open")
        .read_to_end(&mut read)
        .await
        .expect("reading a spool back must succeed");
    assert_eq!(
        read, CIPHERTEXT,
        "what is at the path is the Container that was written last, and only it",
    );
}
