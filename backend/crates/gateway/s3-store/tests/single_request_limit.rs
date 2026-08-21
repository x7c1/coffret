//! What happens to a body larger than one request may carry.
//!
//! Every write this gateway makes is a single `PutObject`, which S3 caps at 5
//! GB. The refusal has to come before the request does: the alternative is
//! streaming gigabytes to S3 and being told at the end, which costs a transfer
//! and reports the failure as whatever `EntityTooLarge` happens to translate
//! to. So these cases ask what went on the wire, not what came back.
//!
//! Nothing here allocates five gigabytes. A `ByteStream` carries its length
//! separately from its reader, which is exactly the property the check rests
//! on — so a stream can declare a length it has no bytes for, and a check that
//! reads the stream to find out would fail these cases rather than pass them.

use coffret_usecase::{ByteStream, CommitSlot, Error, ObjectStore};
use s3_store::SINGLE_REQUEST_MAX_BYTES;

mod support;
use support::captured_store;

/// A body that says it is `len` bytes long without holding any of them.
fn claiming(len: u64) -> ByteStream {
    ByteStream::new(len, tokio::io::empty())
}

#[tokio::test]
async fn an_object_too_large_for_one_request_is_refused_before_one_is_made() {
    let (sent, store) = captured_store();

    let error = store
        .put(
            "0123456789abcdef0123456789abcdef.cfrt",
            claiming(SINGLE_REQUEST_MAX_BYTES + 1),
        )
        .await
        .expect_err("a body past the cap cannot be sent as one request");

    let Error::Unsupported { detail } = &error else {
        panic!(
            "an object too large for one request is a request this store cannot serve: {error:?}"
        );
    };
    // What the object would need, rather than what is wrong with it: an
    // oversized singleton Pack is a Pack coffret is meant to make (spec: PK-3),
    // and the limitation is this adapter's until multipart lands.
    assert!(detail.contains("multipart"), "{detail}");

    sent.expect_no_request();
}

#[tokio::test]
async fn a_conditional_create_too_large_for_one_request_is_refused_the_same_way() {
    let (sent, store) = captured_store();

    let error = store
        .put_if_absent(
            &CommitSlot::by_name(),
            "jrn-1.cfrt",
            claiming(SINGLE_REQUEST_MAX_BYTES + 1),
        )
        .await
        .expect_err("a body past the cap cannot be sent as one request");

    assert!(matches!(error, Error::Unsupported { .. }), "{error:?}");
    // And the slot is untouched, because nothing was ever asked of S3: a commit
    // this store could not have made must not look like a commit it lost.
    sent.expect_no_request();
}

#[tokio::test]
async fn a_body_at_the_cap_is_sent_like_any_other() {
    let (sent, store) = captured_store();

    let _ = store
        .put(
            "0123456789abcdef0123456789abcdef.cfrt",
            claiming(SINGLE_REQUEST_MAX_BYTES),
        )
        .await;

    // The cap is the largest object that goes, not the smallest that does not.
    let request = sent.expect_request();
    assert!(
        request
            .uri()
            .contains("/bucket/libraries/alpha/0123456789abcdef0123456789abcdef.cfrt"),
        "unexpected target: {}",
        request.uri(),
    );
    // The object's own length, which is what S3 caps. `Content-Length` is
    // larger than it and says nothing about the cap: the SDK frames the body
    // in signed chunks and carries a trailing checksum, and that framing is
    // what the header measures.
    assert_eq!(
        request.headers().get("x-amz-decoded-content-length"),
        Some(SINGLE_REQUEST_MAX_BYTES.to_string().as_str()),
    );
}
