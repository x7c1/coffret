//! The name of a Library's app folder on Drive, read before there is a store to
//! open.
//!
//! The same stage as [`create_app_folder`](crate::create_app_folder), before a
//! [`GoogleDrive`](crate::GoogleDrive) is built over the folder: here the folder
//! exists, and this device has never been told which Library it holds. That is
//! why the call takes the transport and the token source directly.

use std::sync::Arc;

use coffret_logging::redact::PrivateValues;
use serde::Deserialize;
use tracing::info;

use crate::answer_ceiling::MAX_DOCUMENT_LEN;
use crate::api::{authorization, DriveApi, Endpoints, FailedResponse};
use crate::error::{AppFolderDefect, Error, Result};
use crate::http::{HttpRequest, HttpTransport, Method};
use crate::oauth::AccessTokens;

/// The whole of what the read of a folder's name asks Drive for.
///
/// Not [`FileResource`](crate::api::FileResource), which is what the store reads about an object it may
/// act on: that one requires the id, and the id is what this caller already
/// has. Asking for a field to satisfy a type is how a call ends up carrying
/// what nothing needs.
#[derive(Deserialize)]
struct NamedFile {
    /// The name Drive holds for the file, absent where Drive answered without
    /// one.
    name: Option<String>,
}

/// What the read of an existing folder's name is recorded and reported as.
const READ_OPERATION: &str = "read_app_folder_name";

/// Reports what Drive calls the folder at `folder_id`.
///
/// The other direction of the same fact the create writes down: a device
/// joining a Library it did not create is given the folder's id and nothing
/// else, and the folder's *name* is what says which Library lives in it
/// (spec: FM-18). Reading it is a question about one file this application
/// created, so the `drive.file` grant covers it.
///
/// The name comes back as Drive spells it, defect and all. Whether
/// `coffret-<library id>` is a shape it has is the caller's to decide: this
/// crate knows folders and ids, and which names name a Library is the layer
/// above's vocabulary.
pub async fn read_app_folder_name(
    transport: Arc<dyn HttpTransport>,
    tokens: Arc<dyn AccessTokens>,
    folder_id: &str,
) -> Result<String> {
    let unreadable = |cause| Error::AppFolderUnreadable {
        folder_id: folder_id.to_owned(),
        cause,
    };

    // Only the name is asked for: the id is what the caller already has, and a
    // folder's other fields say nothing about which Library it holds.
    let api = DriveApi::new(transport, tokens, Endpoints::default());
    let url = format!("{}?fields=name", api.endpoints().file(folder_id));
    let response = api
        .send(|token| {
            let (header, value) = authorization(token);
            HttpRequest::new(Method::Get, &url).with_header(header, value)
        })
        .await
        .map_err(|cause| unreadable(AppFolderDefect::Call(cause)))?;

    if !response.is_success() {
        // Nothing private here: the folder is the Library's own app folder,
        // named after the Library ID, and EL-5 leaves that name and the id
        // Drive minted for it as permitted evidence. The folder somebody chose
        // is the create's parent, not this one.
        let cause = FailedResponse::read(response, READ_OPERATION, &PrivateValues::none())
            .await
            .into_object_error(folder_id);
        return Err(unreadable(AppFolderDefect::Call(cause)));
    }

    let body = response
        .into_body()
        .into_bytes_within(MAX_DOCUMENT_LEN)
        .await
        .map_err(|cause| unreadable(AppFolderDefect::Call(cause)))?;
    let file: NamedFile = serde_json::from_slice(&body)
        .map_err(|cause| unreadable(AppFolderDefect::Answer(cause)))?;

    // A resource with no name in it is an answer this build cannot read rather
    // than a folder called nothing: the field was asked for by name.
    let Some(name) = file.name else {
        return Err(unreadable(AppFolderDefect::Nameless));
    };

    // The id is opaque and is what says which folder this was about. The name
    // is not written down: what comes back is whatever Drive holds, and a
    // person is free to rename a folder there into anything at all, so this
    // side of the pair records its length the way every other name-shaped
    // value in this workspace does. The create records the name it composed,
    // which is `coffret-<library id>` and nobody else's wording.
    info!(
        operation = READ_OPERATION,
        name_len = name.len(),
        folder_id = %folder_id,
        "read the name of a Library's app folder"
    );
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    use coffret_logging::testing::CapturedLogs;
    use tracing::Level;

    use crate::http::{StubAnswer, StubTransport};
    use crate::test_support::CountingTokens;

    /// A name of the shape a Library's folder is created under (spec: FM-18).
    const FOLDER_NAME: &str = "coffret-0123456789abcdef";

    // What a joining device is given is the id, and the name is what says which
    // Library the folder holds (spec: FM-18).
    #[tokio::test]
    async fn a_folder_reports_the_name_drive_holds_for_it() {
        let transport = StubTransport::new([StubAnswer::json(
            200,
            &format!(r#"{{"name":"{FOLDER_NAME}"}}"#),
        )]);
        let tokens = CountingTokens::new();

        let name = read_app_folder_name(transport.clone(), tokens, "folder-1")
            .await
            .expect("Drive answered with the folder's name");

        assert_eq!(name, FOLDER_NAME);
        let request = transport.request(0);
        assert!(matches!(request.method, Method::Get));
        assert!(
            request.url.contains("folder-1") && request.url.contains("fields=name"),
            "the read asks Drive for that folder's name: {}",
            request.url
        );
    }

    // A folder somebody renamed on Drive: the id says which folder was read,
    // and the length says enough about the answer to tell an empty one from a
    // plausible name.
    #[tokio::test]
    async fn the_name_drive_answered_with_is_not_written_to_the_log() {
        let logs = CapturedLogs::capture();
        let renamed = "Wedding photos, do not delete";
        let transport =
            StubTransport::new([StubAnswer::json(200, &format!(r#"{{"name":"{renamed}"}}"#))]);
        let tokens = CountingTokens::new();

        let name = read_app_folder_name(transport, tokens, "folder-1")
            .await
            .expect("Drive answered with the folder's name");

        // It is still the answer: what the caller does with it is the layer
        // above's, and only the record of the call leaves the name out.
        assert_eq!(name, renamed);

        let event = logs.only(Level::INFO);
        assert_eq!(event.number("name_len"), renamed.len() as i64);
        assert_eq!(event.field("folder_id"), "folder-1");
        logs.assert_free_of(&[renamed]);
    }

    // The name is the one field asked for, so an answer without it is an answer
    // this build cannot read rather than a folder called nothing.
    #[tokio::test]
    async fn a_folder_answering_with_no_name_is_not_read_as_one() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{"id":"folder-1"}"#)]);
        let tokens = CountingTokens::new();

        let error = read_app_folder_name(transport, tokens, "folder-1")
            .await
            .expect_err("an answer carrying no name cannot report one");

        assert!(
            matches!(
                error,
                Error::AppFolderUnreadable {
                    cause: AppFolderDefect::Nameless,
                    ..
                }
            ),
            "expected a nameless answer, got {error:?}"
        );
    }

    // A folder id that names nothing this application may read is the ordinary
    // way a joining device gets the id wrong, and the refusal names it.
    #[tokio::test]
    async fn a_folder_that_is_not_there_is_reported_as_missing() {
        let transport = StubTransport::new([StubAnswer::json(
            404,
            r#"{"error":{"message":"File not found: folder-1.","errors":[{"reason":"notFound"}]}}"#,
        )]);
        let tokens = CountingTokens::new();

        let error = read_app_folder_name(transport, tokens, "folder-1")
            .await
            .expect_err("a folder that is not there has no name");

        let Error::AppFolderUnreadable {
            folder_id,
            cause: AppFolderDefect::Call(coffret_usecase::Error::NotFound { missing }),
        } = &error
        else {
            panic!("expected the folder to be reported as missing, got {error:?}");
        };
        assert_eq!(folder_id, "folder-1");
        // The app folder is the Library's own, named after the Library ID, so
        // the id Drive minted for it is evidence the report keeps.
        assert_eq!(missing.subject(), "folder-1");
    }
}
