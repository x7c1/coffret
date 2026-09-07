use crate::spool_conformance::spool_under_test::SpoolUnderTest;

/// Removing a spool that is already gone is success (spec: OC-6).
///
/// The rule the whole of local cleanup rests on. A pending row may name a file
/// whose creation never happened, and a cleanup may be interrupted half way
/// through, so disposal has to treat absence as the outcome it wanted — or the
/// ordering that keeps every spool named would open a state no run could get
/// past.
pub async fn a_discard_of_a_spool_that_is_already_gone_succeeds(fixture: &SpoolUnderTest) {
    let spool = fixture.spool();
    let path = fixture.dir().join("a-container.spool");
    spool
        .prepare_dir(fixture.dir())
        .await
        .expect("preparing the spool directory must succeed");

    spool
        .discard(&path)
        .await
        .expect("a spool that was never created is already disposed of");

    let writer = spool.create(&path).await.expect("creating must succeed");
    writer.finish().await.expect("flushing must succeed");
    spool
        .discard(&path)
        .await
        .expect("removing a spool must succeed");
    spool
        .discard(&path)
        .await
        .expect("removing it again must succeed too");
}
