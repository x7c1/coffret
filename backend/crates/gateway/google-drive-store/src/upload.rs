use coffret_logging::redact;
use coffret_usecase::{ByteStream, Error, ObjectRef, Result};
use serde_json::Value;
use tracing::{info, warn};

use crate::api::{authorization, DriveApi, FailedResponse, FileResource, FILE_FIELDS};
use crate::http::{HttpRequest, Method};
use crate::upload_digest::UploadDigest;

/// How a refusal at either step of an upload is read.
///
/// The two creates differ only here: an unconditional `put` has no race to
/// lose, while a conditional one reads a duplicate identifier as another writer
/// having committed first.
pub type TranslateFailure = fn(FailedResponse, &str) -> Error;

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
/// failed write and a Library that is quietly missing a file.
pub async fn create(
    api: &DriveApi,
    operation: &'static str,
    name: &str,
    metadata: Value,
    body: ByteStream,
    translate: TranslateFailure,
) -> Result<ObjectRef> {
    let bytes = body.len();
    let session = open_session(api, operation, name, &metadata, bytes, translate).await?;
    let file = send_bytes(api, operation, name, &session, body, translate).await?;

    // An object reaching Storage whole is the ordinary progress of a run, and
    // the count and size of what went up is what a person compares against what
    // they expected to go up. The name is opaque and the size is of ciphertext,
    // so neither says anything about what was stored.
    info!(operation, object = name, bytes, "stored an object");
    Ok(ObjectRef::new(file.id))
}

/// Opens the upload session and reports where to send the bytes.
async fn open_session(
    api: &DriveApi,
    operation: &'static str,
    name: &str,
    metadata: &Value,
    len: u64,
    translate: TranslateFailure,
) -> Result<String> {
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
        return Err(translate(
            FailedResponse::read(response, operation).await,
            name,
        ));
    }

    response
        .header("location")
        .map(str::to_owned)
        .ok_or_else(|| {
            warn!(
                operation,
                object = name,
                "Storage opened an upload session with nowhere to send the bytes"
            );
            Error::MalformedResponse {
                detail: "the upload session carries no Location to send bytes to".to_owned(),
            }
        })
}

/// Sends the bytes and checks Drive stored the ones that were sent.
async fn send_bytes(
    api: &DriveApi,
    operation: &'static str,
    name: &str,
    session: &str,
    body: ByteStream,
    translate: TranslateFailure,
) -> Result<FileResource> {
    let len = body.len();
    let digest = UploadDigest::new();
    let hashed = ByteStream::new(len, digest.wrap(body.into_reader()));

    let response = api
        .send_once(move |token| {
            let (header, value) = authorization(token);
            HttpRequest::new(Method::Put, session)
                .with_header(header, value)
                .with_header("content-type", "application/octet-stream")
                .with_stream(hashed)
        })
        .await?;

    if !response.is_success() {
        return Err(translate(
            FailedResponse::read(response, operation).await,
            name,
        ));
    }

    let body = response.into_body().into_bytes().await?;
    let file: FileResource = serde_json::from_slice(&body).map_err(|error| {
        warn!(
            operation,
            object = name,
            detail = %error,
            body = %redact::body(&body),
            "Storage answered an upload with something this build cannot read"
        );
        Error::MalformedResponse {
            detail: format!("unreadable file resource: {error}"),
        }
    })?;

    let sent = digest.to_hex();
    match &file.md5_checksum {
        Some(stored) if stored.eq_ignore_ascii_case(&sent) => Ok(file),
        Some(stored) => Err(Error::IntegrityMismatch {
            expected: sent,
            actual: stored.clone(),
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
            })
        }
    }
}
