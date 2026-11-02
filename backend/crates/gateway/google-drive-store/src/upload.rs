use coffret_logging::redact::{self, PrivateValues};
use coffret_usecase::{
    ByteStream, Error, GatewayFailure, ObjectRef, ProviderHash, Result, UploadedObject,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::answer_ceiling::MAX_DOCUMENT_LEN;
use crate::api::{
    authorization, DriveApi, FailedResponse, FileResource, UploadSession, FILE_FIELDS,
};
use crate::error::UnreadableAnswer;
use crate::http::{HttpRequest, Method};
use crate::upload_digest::UploadDigest;

/// How a refusal at either step of an upload is read.
///
/// The two creates differ only here: an unconditional `put` has no race to
/// lose, while a conditional one reads a duplicate identifier as another writer
/// having committed first.
pub type ClassifyFailure = fn(FailedResponse, &str) -> Error;

/// Creates one file, streaming its bytes through a resumable session.
///
/// Drive's resumable upload is used for every object, whatever its size. It is
/// what lets a large Container go up without being held in memory, and using
/// one path for everything means the upload that carries a Journal record is
/// the same code as the one that carries a photo library.
///
/// The bytes are hashed as they are sent, and the digest Drive reports is
/// checked against them before the upload counts as successful. An object that
/// arrived corrupted or short is not a Storage Object coffret would ever be able
/// to open, and finding that out at upload time is the difference between a
/// failed write and a Library that is quietly missing a file. The digest Drive
/// reported travels on in the answer, so the caller can hold the object to it
/// as well.
///
/// The bytes, and the token with them, go only to a session URI at Drive's own
/// upload endpoint ([`UploadSession`]), and a redirect answering them is a
/// failure rather than somewhere else to send them. A session URI anywhere else
/// is refused before a token is attached or a byte of the body is read.
pub async fn create(
    api: &DriveApi,
    operation: &'static str,
    name: &str,
    metadata: Value,
    body: ByteStream,
    classify: ClassifyFailure,
) -> Result<UploadedObject> {
    let bytes = body.len();
    let session = open_session(api, operation, name, &metadata, bytes, classify).await?;
    let (file, stored) = send_bytes(api, operation, name, &session, body, classify).await?;

    // An object reaching Storage whole is the ordinary progress of a run, and
    // the count and size of what went up is what a person compares against what
    // they expected to go up. The name is one coffret minted and the size is
    // of ciphertext, so neither names a file or a location.
    info!(operation, object = name, bytes, "stored an object");
    Ok(UploadedObject {
        object_ref: ObjectRef::new(file.id),
        hash: Some(ProviderHash::new(stored)),
    })
}

/// Opens the upload session and reports where to send the bytes.
///
/// Where Drive said to send them is taken only if it is Drive's own upload
/// endpoint. Anything else is a malformed answer from Storage — not a transport
/// failure, and nothing a retry would change — and is reported by the rule it
/// broke, never by the URL itself: its query is the session's capability.
async fn open_session(
    api: &DriveApi,
    operation: &'static str,
    name: &str,
    metadata: &Value,
    len: u64,
    classify: ClassifyFailure,
) -> Result<UploadSession> {
    let url = format!(
        "{}?uploadType=resumable&fields={FILE_FIELDS}",
        api.endpoints().upload()
    );

    let response = api
        .send(|token| {
            let (header, value) = authorization(token);
            HttpRequest::new(Method::Post, &url)
                .with_header(header, value)
                // Telling Drive the size up front is what lets it refuse an
                // object too large for the account before any bytes move.
                .with_header("x-upload-content-length", len.to_string())
                .with_header("x-upload-content-type", "application/octet-stream")
                .with_json(metadata)
        })
        .await?;

    if !response.is_success() {
        return Err(classify(
            FailedResponse::read(response, operation, &PrivateValues::none()).await,
            name,
        ));
    }

    let Some(location) = response.header("location") else {
        warn!(
            operation,
            object = name,
            "Storage opened an upload session with nowhere to send the bytes"
        );
        return Err(Error::MalformedResponse {
            detail: "the upload session carries no Location to send bytes to".to_owned(),
            source: None,
        });
    };

    UploadSession::accept(api.endpoints(), location).map_err(|refusal| {
        warn!(
            operation,
            object = name,
            refusal = %refusal,
            "Storage opened an upload session somewhere other than its own upload endpoint, so no bytes were sent"
        );
        Error::MalformedResponse {
            // The refusal is the next link, so the rule it broke reads under
            // this line rather than folded into it.
            detail: "the upload session is not at Drive's upload endpoint".to_owned(),
            source: Some(GatewayFailure::new(refusal)),
        }
    })
}

/// Sends the bytes and checks Drive stored the ones that were sent, answering
/// with the file resource and the digest Drive reported for it.
async fn send_bytes(
    api: &DriveApi,
    operation: &'static str,
    name: &str,
    session: &UploadSession,
    body: ByteStream,
    classify: ClassifyFailure,
) -> Result<(FileResource, String)> {
    let len = body.len();
    let digest = UploadDigest::new();
    let hashed = ByteStream::new(len, digest.wrap(body.into_reader()));

    let response = api
        .send_once(move |token| {
            let (header, value) = authorization(token);
            HttpRequest::new(Method::Put, session.as_str())
                .with_header(header, value)
                .with_header("content-type", "application/octet-stream")
                .with_stream(hashed)
                // Followed, a redirect would carry the token and the object on
                // to wherever it pointed, past the check the session passed.
                .not_following_redirects()
        })
        .await?;

    // A 3xx lands here too, and is read like any other answer that is not the
    // file resource.
    if !response.is_success() {
        return Err(classify(
            FailedResponse::read(response, operation, &PrivateValues::none()).await,
            name,
        ));
    }

    // What comes back from a finished upload is the file resource, whose fields
    // the request named — not the object that was just sent.
    let body = response
        .into_body()
        .into_bytes_within(MAX_DOCUMENT_LEN)
        .await?;
    let file: FileResource = serde_json::from_slice(&body).map_err(|error| {
        warn!(
            operation,
            object = name,
            detail = %error,
            body = %redact::body_without(&body, &PrivateValues::none()),
            "Storage answered an upload with something this build cannot read"
        );
        UnreadableAnswer::new("file resource", error).into_port()
    })?;

    let sent = digest.to_hex();
    match file.md5_checksum.clone() {
        Some(stored) if stored.eq_ignore_ascii_case(&sent) => Ok((file, stored)),
        Some(stored) => Err(Error::IntegrityMismatch {
            expected: sent,
            actual: stored,
        }),
        // The field was asked for, so its absence means the answer is not one
        // this build can verify — and an unverified upload is not a successful
        // one.
        None => {
            warn!(
                operation,
                object = name,
                "Storage reported no digest for an upload, so nothing confirms it arrived whole"
            );
            Err(Error::MalformedResponse {
                detail: format!("Storage reported no digest for {name:?}"),
                source: None,
            })
        }
    }
}
