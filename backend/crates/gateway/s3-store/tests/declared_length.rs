//! What a read makes of an answer that does not say how long the body is.
//!
//! S3 states the length of every `GetObject` body it answers with, so an answer
//! that states none is not an object of no bytes — it is something other than
//! S3 answering.

use coffret_usecase::{Error, ObjectRef, ObjectStore};

mod support;
use support::captured_store;

#[tokio::test]
async fn a_read_answered_without_a_content_length_is_a_malformed_response() {
    // The captured client's stand-in answer is a bare 200 carrying no body and
    // no length, which is exactly the shape in question.
    let (_sent, store) = captured_store();

    let error = store
        .get(&ObjectRef::new("head-1.cfrt"), None)
        .await
        .expect_err("an answer stating no length is not a body of no bytes");

    assert!(
        matches!(error, Error::MalformedResponse { .. }),
        "{error:?}"
    );
}
