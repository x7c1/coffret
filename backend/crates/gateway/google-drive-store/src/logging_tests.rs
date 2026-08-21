//! What reaches the log, and what must never reach it.
//!
//! Drive's real behaviour is the thing this gateway cannot know in advance —
//! what an account over its daily upload cap answers with, whether a minted
//! identifier expires — so the answers that fall into a catch-all are recorded
//! as they arrived. These cases prove they are, against the scripted transport
//! rather than a live account, and prove the other half too: that nothing
//! carrying a credential ever goes with them.

use std::sync::Arc;

use coffret_logging::testing::CapturedLogs;
use coffret_model::MasterKey;
use coffret_usecase::{ByteStream, Error, ObjectStore};
use tracing::Level;

use crate::http::{HttpTransport, StubAnswer, StubTransport};
use crate::test_support::{
    scripted_drive, session_opened, upload_finished, CIPHERTEXT, CIPHERTEXT_MD5,
};
use crate::{AccessTokens, ClientCredentials, OAuthTokens, StoredTokens, TokenCache};

/// A refusal Drive gives for a reason the port has no state for.
const NO_PERMISSION: &str = r#"{"error":{"message":"The user does not have sufficient permissions for this file.","errors":[{"reason":"insufficientFilePermissions"}]}}"#;

/// A refusal of a kind nobody has seen before, which is the case that matters.
const UNFAMILIAR: &str =
    r#"{"error":{"message":"Upload cap reached.","errors":[{"reason":"someUndocumentedReason"}]}}"#;

/// Drive's answer when nothing is stored under the name.
const NOT_FOUND: &str =
    r#"{"error":{"message":"File not found.","errors":[{"reason":"notFound"}]}}"#;

/// Drive's answer to a token it will not accept.
const REJECTED: &str =
    r#"{"error":{"message":"Invalid Credentials","errors":[{"reason":"authError"}]}}"#;

/// The error a listing comes back with when Drive answers this.
async fn listing_error(answer: StubAnswer) -> Error {
    let (store, _, _) = scripted_drive([answer]);
    store
        .list(None)
        .await
        .expect_err("the scripted answer must fail the call")
}

#[tokio::test]
async fn a_refusal_the_port_has_no_state_for_is_recorded_as_it_arrived() {
    let logs = CapturedLogs::capture();

    let error = listing_error(StubAnswer::json(403, NO_PERMISSION)).await;
    assert!(matches!(error, Error::PermissionDenied { .. }), "{error:?}");

    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "list");
    assert_eq!(event.number("status"), 403);
    assert_eq!(event.field("reason"), "insufficientFilePermissions");
    assert!(
        event
            .field("body")
            .contains("The user does not have sufficient permissions"),
        "the body Drive answered with is the evidence: {event}",
    );
}

#[tokio::test]
async fn a_reason_nobody_has_seen_before_survives_in_the_body_it_arrived_in() {
    let logs = CapturedLogs::capture();

    let error = listing_error(StubAnswer::json(400, UNFAMILIAR)).await;
    assert!(
        matches!(error, Error::Rejected { status: 400, .. }),
        "{error:?}"
    );

    let event = logs.only(Level::WARN);
    assert_eq!(event.number("status"), 400);
    assert_eq!(
        event.field("reason"),
        "someUndocumentedReason",
        "the reason is the whole point of the record: {event}",
    );
    // And in the body as well, whole: a reason this build has no name for is
    // read out of an answer nobody has seen the shape of.
    assert!(
        event.field("body").contains("someUndocumentedReason"),
        "{event}",
    );
    assert!(
        event.field("body").contains("Upload cap reached."),
        "{event}"
    );
}

#[tokio::test]
async fn a_missing_object_is_not_a_failure_anybody_has_to_act_on() {
    let logs = CapturedLogs::capture();

    let error = listing_error(StubAnswer::json(404, NOT_FOUND)).await;
    assert!(matches!(error, Error::NotFound { .. }), "{error:?}");

    assert!(
        logs.at(Level::ERROR).is_empty(),
        "a missing object is ordinary: {}",
        logs.text(),
    );
    assert!(
        logs.at(Level::WARN).is_empty(),
        "and not a warning either: {}",
        logs.text(),
    );
}

#[tokio::test]
async fn an_answer_this_build_cannot_read_is_recorded_with_what_was_unreadable() {
    let logs = CapturedLogs::capture();
    let (store, _, _) = scripted_drive([StubAnswer::json(200, "not a listing at all")]);

    let error = store
        .list(None)
        .await
        .expect_err("an unreadable answer must fail the call");
    assert!(
        matches!(error, Error::MalformedResponse { .. }),
        "{error:?}"
    );

    let event = logs.only(Level::WARN);
    assert!(
        event.field("body").contains("not a listing at all"),
        "{event}"
    );
}

#[tokio::test]
async fn a_create_that_could_not_have_lost_a_race_finding_the_name_taken_is_recorded() {
    let logs = CapturedLogs::capture();
    let (store, _, _) = scripted_drive([StubAnswer::json(
        409,
        r#"{"error":{"message":"A file already exists with that id.","errors":[{"reason":"duplicate"}]}}"#,
    )]);

    // An unconditional `put`: there is no commit slot, so there is no race to
    // have lost, and Drive refusing it as taken contradicts that.
    let error = store
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect_err("a conflict must fail an unconditional create");
    assert!(
        matches!(error, Error::Rejected { status: 409, .. }),
        "{error:?}"
    );

    let event = logs.only(Level::WARN);
    assert!(
        event.message().contains("could not have lost a race"),
        "{event}",
    );
}

#[tokio::test]
async fn a_lost_commit_race_is_the_protocol_working_and_is_not_warned_about() {
    let logs = CapturedLogs::capture();
    let (store, _, _) = scripted_drive([StubAnswer::json(
        409,
        r#"{"error":{"message":"A file already exists with that id.","errors":[{"reason":"duplicate"}]}}"#,
    )]);

    let error = store
        .put_if_absent(
            &coffret_usecase::CommitSlot::provider_id("reserved-1"),
            "jrn-1.cfrt",
            ByteStream::from(CIPHERTEXT),
        )
        .await
        .expect_err("a taken slot must not accept a second object");
    assert!(matches!(error, Error::AlreadyExists { .. }), "{error:?}");

    assert!(logs.at(Level::WARN).is_empty(), "{}", logs.text());
    assert!(logs.at(Level::ERROR).is_empty(), "{}", logs.text());
}

#[tokio::test]
async fn giving_up_after_the_one_retry_there_is_says_how_many_attempts_it_took() {
    let logs = CapturedLogs::capture();
    let (store, _, _) = scripted_drive([
        StubAnswer::json(401, REJECTED),
        StubAnswer::json(401, REJECTED),
    ]);

    let error = store
        .list(None)
        .await
        .expect_err("a grant that is gone must fail the call");
    assert!(matches!(error, Error::Unauthenticated { .. }), "{error:?}");

    let event = logs.only(Level::WARN);
    assert_eq!(event.number("attempts"), 2);
    assert!(event.message().contains("gave up"), "{event}");
}

#[tokio::test]
async fn an_object_that_reached_storage_is_recorded_as_progress() {
    let logs = CapturedLogs::capture();
    let (store, _, _) = scripted_drive([session_opened(), upload_finished(Some(CIPHERTEXT_MD5))]);

    store
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect("the upload must succeed");

    let event = logs.only(Level::INFO);
    assert_eq!(event.message(), "stored an object");
    assert_eq!(event.field("object"), "jrn-1.cfrt");
    assert_eq!(event.number("bytes"), CIPHERTEXT.len() as i64);
}

#[tokio::test]
async fn an_individual_call_is_detail_rather_than_progress() {
    let logs = CapturedLogs::capture();
    let (store, _, _) = scripted_drive([StubAnswer::json(200, r#"{"files":[]}"#)]);

    store.list(None).await.expect("the call must succeed");

    let event = logs.only(Level::DEBUG);
    assert_eq!(event.field("method"), "Get");
    assert_eq!(event.number("status"), 200);
}

#[tokio::test]
async fn the_upload_session_drive_minted_never_reaches_the_log() {
    let logs = CapturedLogs::capture();
    let (store, transport, _) =
        scripted_drive([session_opened(), upload_finished(Some(CIPHERTEXT_MD5))]);

    store
        .put("jrn-1.cfrt", ByteStream::from(CIPHERTEXT))
        .await
        .expect("the upload must succeed");

    // The bytes really were sent to the session Drive handed back, so the
    // absence below is the gateway cutting it out rather than there having been
    // nothing to cut.
    assert!(
        transport.request(1).url.contains("upload_id=session-1"),
        "{:?}",
        transport.request(1).url,
    );
    // Whoever holds an `upload_id` can write to that upload, which makes it a
    // credential in everything but name.
    logs.assert_free_of(&["upload_id", "session-1"]);

    // The endpoint the bytes went to is what the record was for, and it
    // survives being cut down to it.
    let calls = logs.at(Level::DEBUG);
    assert!(
        calls
            .iter()
            .any(|event| event.field("url").contains("upload/drive/v3/files")),
        "{calls:#?}",
    );
}

#[tokio::test]
async fn no_access_token_and_no_authorization_header_reaches_the_log() {
    let logs = CapturedLogs::capture();
    let (store, transport, _) = scripted_drive([StubAnswer::json(403, NO_PERMISSION)]);

    let _ = store.list(None).await;

    // The call really did carry one, so the absence below is the gateway
    // keeping it out rather than there having been nothing to keep out.
    assert_eq!(
        transport.request(0).header("authorization"),
        Some("Bearer token-0"),
    );
    logs.assert_free_of(&["token-0", "Bearer", "authorization"]);
}

#[tokio::test]
async fn nothing_the_oauth_path_holds_reaches_the_log() {
    // The one path in this gateway that handles a refresh token, a Master Key,
    // and the name of a local file. None of the three may be recorded, and the
    // refusal that path met still has to be.
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let path = directory.path().join("tokens.bin");
    let key = MasterKey::from_bytes([0x3d; MasterKey::BYTE_LEN]);
    let cache = TokenCache::new(&path, key);
    cache
        .store(&StoredTokens {
            refresh_token: "1//0gSecretRefreshToken".to_owned(),
        })
        .expect("storing must succeed");

    let logs = CapturedLogs::capture();
    let transport: Arc<dyn HttpTransport> = StubTransport::new([StubAnswer::json(
        400,
        r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#,
    )]);
    let tokens = OAuthTokens::new(
        transport,
        ClientCredentials::new("an-app.apps.googleusercontent.com")
            .with_client_secret("a-client-secret"),
        cache,
    );

    let error = tokens
        .access_token()
        .await
        .expect_err("a revoked grant must fail");
    // Asked by variant and by the field the endpoint's answer travels in,
    // rather than by how the whole error renders: what has to survive the
    // redaction above is Google's own `invalid_grant`, in the refusal the
    // gateway classified this as.
    let crate::Error::TokenEndpoint { status, detail } = &error else {
        panic!("a revoked grant must come back as the endpoint's refusal: {error:?}");
    };
    assert_eq!(*status, 400);
    assert!(detail.contains("invalid_grant"), "{detail}");

    let event = logs.only(Level::WARN);
    assert_eq!(event.number("status"), 400);
    assert!(event.field("body").contains("invalid_grant"), "{event}");

    logs.assert_free_of(&[
        // The refresh token, which the request carried.
        "1//0gSecretRefreshToken",
        "0gSecret",
        // The client secret, which it carried too.
        "a-client-secret",
        // The Master Key the cache is sealed under, however it might be
        // rendered.
        "3d3d3d3d",
        "MasterKey",
        // A local file name, which says where a person keeps their things.
        &path.display().to_string(),
        "tokens.bin",
    ]);
}

#[tokio::test]
async fn emitting_with_nothing_installed_changes_nothing() {
    // Nothing is installed on this thread, and no test in this binary installs
    // a subscriber for the process: a library crate emits and never decides
    // where events go.
    let without = listing_error(StubAnswer::json(403, NO_PERMISSION)).await;

    let logs = CapturedLogs::capture();
    let with = listing_error(StubAnswer::json(403, NO_PERMISSION)).await;

    // Compared by what the failure means — the variant, and the detail that
    // travels in it — rather than by the whole value: an error type is not
    // asked whether it is equal to another, so that a field added to it later
    // is not a change to what these two runs are being held to.
    match (without, with) {
        (Error::PermissionDenied { detail: without }, Error::PermissionDenied { detail: with }) => {
            assert_eq!(without, with)
        }
        (without, with) => {
            panic!("the refusal did not survive being recorded: {without:?} became {with:?}")
        }
    }
    assert!(
        !logs.text().is_empty(),
        "the path has to emit something, or this case proves nothing",
    );
}
