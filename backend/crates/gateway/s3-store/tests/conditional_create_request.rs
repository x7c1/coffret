//! What a conditional create puts on the wire.
//!
//! The suite proves against a running S3 that a second create loses; this
//! proves what makes it lose. Without `If-None-Match: *` the second PUT would
//! quietly overwrite the winner, and a Journal commit would stop being a commit
//! — a regression a live test could only catch on a provider that happens to
//! reject the write for some other reason.

use coffret_usecase::{ByteStream, CommitSlot, ObjectStore};

mod support;
use support::captured_store;

#[tokio::test]
async fn a_conditional_create_refuses_to_overwrite_anything() {
    let (sent, store) = captured_store();

    let _ = store
        .put_if_absent(
            &CommitSlot::by_name(),
            "jrn-1.cfrt",
            ByteStream::from(b"the first Journal record".to_vec()),
        )
        .await;

    let request = sent.expect_request();
    assert_eq!(request.headers().get("if-none-match"), Some("*"));
    assert!(
        request.uri().contains("/bucket/libraries/alpha/jrn-1.cfrt"),
        "unexpected target: {}",
        request.uri()
    );
}

#[tokio::test]
async fn an_ordinary_put_carries_no_condition() {
    let (sent, store) = captured_store();

    let _ = store
        .put(
            "0123456789abcdef0123456789abcdef.cfrt",
            ByteStream::from(b"a Container".to_vec()),
        )
        .await;

    let request = sent.expect_request();
    assert_eq!(request.headers().get("if-none-match"), None);
}
