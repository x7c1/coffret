//! What an upload has to prove before it counts.
//!
//! An object that arrived at Storage corrupted or short is one coffret will
//! never be able to open, and the moment to find that out is while the upload
//! is still a failure that can be retried — not months later, when a Container
//! turns out not to authenticate. Drive reports an MD5 of what it stored; the
//! gateway hashes the bytes as it sends them and refuses to call the upload
//! successful unless the two agree.
//!
//! Before any of that, the bytes have to go to Drive at all. The session URI
//! they are sent to is whatever Drive answered the opening with, so an answer
//! replaced on its way back could point them, and the bearer token with them,
//! anywhere; the gateway sends them only to a session at Drive's own upload
//! endpoint, and a redirect answering them is a failure rather than a second
//! place to send them.

use coffret_usecase::{ByteStream, CommitSlot, Error, ObjectStore, ProviderHash};

use std::sync::atomic::Ordering;

use crate::http::{Method, StubAnswer};
use crate::test_support::{
    scripted_drive, session_opened, session_opened_at, upload_finished, WatchedBody, CIPHERTEXT,
    CIPHERTEXT_MD5,
};

#[tokio::test]
async fn an_upload_drive_agrees_with_is_the_object_that_was_sent() {
    let (store, transport, _) =
        scripted_drive([session_opened(), upload_finished(Some(CIPHERTEXT_MD5))]);

    let object = store
        .put("head-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect("an upload whose digest agrees must succeed");

    assert_eq!(object.object_ref.as_str(), "file-1");
    assert_eq!(
        object.hash.as_ref().map(ProviderHash::as_str),
        Some(CIPHERTEXT_MD5),
        "the answer carries the digest Drive reported",
    );
    assert_eq!(transport.request(1).body, CIPHERTEXT);
}

#[tokio::test]
async fn an_upload_drive_disagrees_with_is_not_a_stored_object() {
    let wrong = "00000000000000000000000000000000";
    let (store, _, _) = scripted_drive([session_opened(), upload_finished(Some(wrong))]);

    let error = store
        .put("head-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect_err("a digest that disagrees must fail the upload");

    match &error {
        // Both digests are reported, so a log says what was sent and what Drive
        // claims it stored.
        Error::IntegrityMismatch { expected, actual } => {
            assert_eq!(expected, CIPHERTEXT_MD5);
            assert_eq!(actual, wrong);
        }
        other => panic!("expected a digest mismatch, got {other:?}"),
    }
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn an_upload_drive_says_nothing_about_is_not_taken_on_trust() {
    let (store, _, _) = scripted_drive([session_opened(), upload_finished(None)]);

    let error = store
        .put("head-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect_err("an unverifiable upload must fail");

    assert!(
        matches!(error, Error::MalformedResponse { .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_upload_declares_its_length_before_any_bytes_move() {
    let (store, transport, _) =
        scripted_drive([session_opened(), upload_finished(Some(CIPHERTEXT_MD5))]);

    store
        .put("head-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect("the upload must succeed");

    let opening = transport.request(0);
    assert_eq!(
        opening.header("x-upload-content-length"),
        Some(CIPHERTEXT.len().to_string().as_str())
    );
    assert!(
        opening.url.contains("uploadType=resumable"),
        "unexpected target: {}",
        opening.url
    );
}

#[tokio::test]
async fn a_conditional_create_names_the_identifier_it_reserved() {
    let (store, transport, _) =
        scripted_drive([session_opened(), upload_finished(Some(CIPHERTEXT_MD5))]);

    store
        .put_if_absent(
            &CommitSlot::provider_id("head-1.cfrt", "reserved-1"),
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect("the create must succeed");

    let metadata: serde_json::Value = serde_json::from_slice(&transport.request(0).body)
        .expect("the create must carry JSON metadata");

    assert_eq!(metadata["id"], "reserved-1");
    assert_eq!(metadata["name"], "head-1.cfrt");
    assert_eq!(metadata["parents"], serde_json::json!(["folder-1"]));
}

#[tokio::test]
async fn a_conditional_create_onto_a_taken_identifier_is_a_lost_race() {
    let (store, _, _) = scripted_drive([StubAnswer::json(
        409,
        r#"{"error":{"message":"A file already exists with that id.","errors":[{"reason":"duplicate"}]}}"#,
    )]);

    let error = store
        .put_if_absent(
            &CommitSlot::provider_id("head-1.cfrt", "reserved-1"),
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect_err("a taken identifier must not accept a second object");

    match &error {
        Error::AlreadyExists { object } => assert_eq!(object, "head-1.cfrt"),
        other => panic!("expected a lost conditional create, got {other:?}"),
    }
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_slot_from_a_store_that_keys_by_name_is_refused() {
    let (store, transport, _) = scripted_drive([]);

    let error = store
        .put_if_absent(
            &CommitSlot::by_name("head-1.cfrt"),
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect_err("a slot this store never minted must be refused");

    assert!(matches!(error, Error::Unsupported { .. }), "{error:?}");
    assert_eq!(transport.call_count(), 0, "nothing should have been sent");
}

#[tokio::test]
async fn an_upload_session_anywhere_but_drives_upload_endpoint_gets_neither_token_nor_body() {
    let elsewhere = [
        "https://collector.invalid/upload",
        "http://collector.invalid/upload",
        "http://127.0.0.1:49123/upload",
        "http://www.googleapis.com/upload/drive/v3/files?upload_id=x",
        "https://www.googleapis.com:8443/upload/drive/v3/files?upload_id=x",
        "https://user:pw@www.googleapis.com/upload/drive/v3/files?upload_id=x",
        "https://www.googleapis.com.evil.example/upload/drive/v3/files?upload_id=x",
        "https://www.googleapis.com/upload/drive/v3/files?upload_id=x#frag",
        "https://www.googleapis.com/drive/v3/files?upload_id=x",
        "https://www.googleapis.com/upload/drive/v3/files",
        "not a url",
    ];

    for location in elsewhere {
        let (store, transport, _) = scripted_drive([session_opened_at(location)]);
        let (body, read) = WatchedBody::stream(CIPHERTEXT.len() as u64);

        let error = store
            .put("head-1.cfrt", body)
            .await
            .expect_err("a session anywhere else must be refused");

        assert!(
            matches!(error, Error::MalformedResponse { .. }),
            "{location}: refused as an answer this build cannot use, got {error:?}"
        );
        assert!(!error.is_retryable(), "{location}: a retry changes nothing");
        assert!(
            !read.load(Ordering::SeqCst),
            "{location}: the body was never read"
        );
        // The opening is the one call made, and the one that carried a token:
        // it went to Drive's upload endpoint, not to where the answer pointed.
        assert_eq!(
            transport.call_count(),
            1,
            "{location}: nothing was sent there"
        );
        let opening = transport.request(0);
        assert_eq!(opening.method, Method::Post);
        assert!(
            opening
                .url
                .starts_with("https://www.googleapis.com/upload/drive/v3/files?"),
            "{location}: the token went only to the upload endpoint: {}",
            opening.url
        );
    }
}

#[tokio::test]
async fn an_upload_answered_with_a_redirect_fails_without_following_it() {
    let (store, transport, _) = scripted_drive([
        session_opened(),
        StubAnswer::redirect(307, "https://collector.invalid/upload"),
    ]);

    let error = store
        .put("head-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect_err("a redirect is not a stored object");

    assert!(
        matches!(error, Error::Rejected { status: 307, .. }),
        "classified like any other answer it did not expect: {error:?}"
    );
    assert_eq!(
        transport.call_count(),
        2,
        "the opening and the upload, and nothing sent where the redirect pointed"
    );
    let upload = transport.request(1);
    assert_eq!(upload.method, Method::Put);
    assert!(
        !upload.follow_redirects,
        "the upload asks its transport not to follow a redirect"
    );
    assert!(
        transport.request(0).follow_redirects,
        "the opening keeps following them as every other call does"
    );
}
