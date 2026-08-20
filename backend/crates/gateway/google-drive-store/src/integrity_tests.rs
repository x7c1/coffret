//! What an upload has to prove before it counts.
//!
//! An object that arrived at Storage corrupted or short is one coffret will
//! never be able to open, and the moment to find that out is while the upload
//! is still a failure that can be retried — not months later, when a Container
//! turns out not to authenticate. Drive reports an MD5 of what it stored; the
//! gateway hashes the bytes as it sends them and refuses to call the upload
//! successful unless the two agree.

use coffret_usecase::{ByteStream, CommitSlot, Error, ObjectStore};

use crate::http::StubAnswer;
use crate::test_support::scripted_drive;

/// The MD5 of the bytes every case here uploads.
const CIPHERTEXT_MD5: &str = "cb54616748fddc2fb607b9eb4312ee3d";

/// The bytes every case here uploads.
const CIPHERTEXT: &[u8] = b"ciphertext";

/// Drive's answer to opening a resumable upload session.
fn session_opened() -> StubAnswer {
    StubAnswer::json_with_headers(
        200,
        vec![(
            "location".to_owned(),
            "https://www.googleapis.com/upload/drive/v3/files?upload_id=session-1".to_owned(),
        )],
        "",
    )
}

/// Drive's answer to a finished upload, reporting this digest.
fn upload_finished(md5: Option<&str>) -> StubAnswer {
    let digest = match md5 {
        Some(md5) => format!(r#","md5Checksum":"{md5}""#),
        None => String::new(),
    };
    StubAnswer::json(
        200,
        &format!(r#"{{"id":"file-1","name":"jrn-1.cfrt","size":"10"{digest}}}"#),
    )
}

#[tokio::test]
async fn an_upload_drive_agrees_with_is_the_object_that_was_sent() {
    let (store, transport, _) =
        scripted_drive([session_opened(), upload_finished(Some(CIPHERTEXT_MD5))]);

    let object = store
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect("an upload whose digest agrees must succeed");

    assert_eq!(object.as_str(), "file-1");
    assert_eq!(transport.request(1).body, CIPHERTEXT);
}

#[tokio::test]
async fn an_upload_drive_disagrees_with_is_not_a_stored_object() {
    let wrong = "00000000000000000000000000000000";
    let (store, _, _) = scripted_drive([session_opened(), upload_finished(Some(wrong))]);

    let error = store
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
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
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
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
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
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
            &CommitSlot::provider_id("reserved-1"),
            "jrn-1.cfrt",
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect("the create must succeed");

    let metadata: serde_json::Value = serde_json::from_slice(&transport.request(0).body)
        .expect("the create must carry JSON metadata");

    assert_eq!(metadata["id"], "reserved-1");
    assert_eq!(metadata["name"], "jrn-1.cfrt");
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
            &CommitSlot::provider_id("reserved-1"),
            "jrn-1.cfrt",
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect_err("a taken identifier must not accept a second object");

    match &error {
        Error::AlreadyExists { object } => assert_eq!(object, "jrn-1.cfrt"),
        other => panic!("expected a lost conditional create, got {other:?}"),
    }
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_slot_from_a_store_that_keys_by_name_is_refused() {
    let (store, transport, _) = scripted_drive([]);

    let error = store
        .put_if_absent(
            &CommitSlot::by_name(),
            "jrn-1.cfrt",
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect_err("a slot this store never minted must be refused");

    assert!(matches!(error, Error::Unsupported { .. }), "{error:?}");
    assert_eq!(transport.call_count(), 0, "nothing should have been sent");
}
