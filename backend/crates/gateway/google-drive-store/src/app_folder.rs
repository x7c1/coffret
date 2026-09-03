//! The Library's app folder on Drive, before there is a store to open.
//!
//! Everything else in this crate works inside one Library's folder: the
//! `ObjectStore` port is scoped to a Library, and [`GoogleDrive`] is built from
//! the folder id it is to work in. This module is the stage before that — the
//! folder does not exist yet, or it exists and this device has never been told
//! which Library it holds — which is why both calls here take the transport and
//! the token source directly rather than hanging off either the port or the
//! store.
//!
//! [`GoogleDrive`]: crate::GoogleDrive

use std::sync::Arc;

use coffret_model::LibraryId;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

use crate::answer_ceiling::MAX_DOCUMENT_LEN;
use crate::api::{authorization, DriveApi, Endpoints, FailedResponse, FileResource};
use crate::error::{AppFolderDefect, Error, Result};
use crate::http::{HttpRequest, HttpTransport, Method};
use crate::oauth::AccessTokens;

/// What Drive calls a folder.
const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";

/// The whole of what the read of a folder's name asks Drive for.
///
/// Not [`FileResource`], which is what the store reads about an object it may
/// act on: that one requires the id, and the id is what this caller already
/// has. Asking for a field to satisfy a type is how a call ends up carrying
/// what nothing needs.
#[derive(Deserialize)]
struct NamedFile {
    /// The name Drive holds for the file, absent where Drive answered without
    /// one.
    name: Option<String>,
}

/// What the create is recorded and reported as.
///
/// The same name on the event that says the folder was created and on the one
/// that says it was not, so a reader asking what this operation did sees both.
const OPERATION: &str = "create_app_folder";

/// What the read of an existing folder's name is recorded and reported as.
const READ_OPERATION: &str = "read_app_folder_name";

/// Creates the folder one Library's objects will live in, and reports its id.
///
/// The folder is named `coffret-<library id>` (spec: FM-18), which is what a
/// device recovering with only a Recovery Code enumerates for. `parent` is the
/// folder it goes in, and there is no way to ask for it to go anywhere else:
/// the top of My Drive is not where an application's folder belongs, and `root`
/// does not name it as a parent either — that is an alias for a folder this
/// application did not create, so asking for it asks for access the
/// `drive.file` grant does not cover.
///
/// No retry policy wraps the call, and no answer to it is met by making it
/// again. A folder create is not idempotent and Drive mints the id, so a retry
/// after an answer that was lost on the way back would leave two folders behind
/// — one of them holding the Library and the other one findable by the same
/// enumeration. The one repeat that can happen is the token refresh `DriveApi`
/// makes for every call: a 401 is Drive refusing the token before it created
/// anything, so the create is made once more under a fresh one. A caller that
/// gets a retryable failure here is free to try again knowing nothing was
/// created; one that gets no answer at all has to look before it does.
pub async fn create_app_folder(
    transport: Arc<dyn HttpTransport>,
    tokens: Arc<dyn AccessTokens>,
    parent: &str,
    library: LibraryId,
) -> Result<String> {
    let name = library.app_folder_name();
    let metadata = json!({
        "name": name,
        "mimeType": FOLDER_MIME_TYPE,
        "parents": [parent],
    });

    // Only the id is asked for: it is the whole of what a store is built from,
    // and the rest of the resource says nothing this caller does not already
    // know.
    let api = DriveApi::new(transport, tokens, Endpoints::default());
    let url = format!("{}?fields=id", api.endpoints().files());
    let response = api
        .send(|token| {
            let (header, value) = authorization(token);
            HttpRequest::new(Method::Post, &url)
                .with_header(header, value)
                .with_json(&metadata)
        })
        .await
        .map_err(|cause| failed(&name, AppFolderDefect::Call(cause)))?;

    if !response.is_success() {
        // What a refusal can report as missing is the folder the new one was to
        // go in, never the new one: that is the one thing which certainly does
        // not exist yet, and naming it would turn "the configured folder is
        // gone" into "the Library's folder is gone".
        let cause = FailedResponse::read(response, OPERATION)
            .await
            .into_error(parent);
        return Err(failed(&name, AppFolderDefect::Call(cause)));
    }

    let body = response
        .into_body()
        .into_bytes_within(MAX_DOCUMENT_LEN)
        .await
        .map_err(|cause| failed(&name, AppFolderDefect::Call(cause)))?;
    let created: FileResource = serde_json::from_slice(&body)
        .map_err(|cause| failed(&name, AppFolderDefect::Answer(cause)))?;

    // Worth keeping for the life of the Library rather than only while a run is
    // being investigated: this is the one moment the folder every later call
    // works in came into being, and its name is the Library's own.
    info!(
        operation = OPERATION,
        folder = %name,
        folder_id = %created.id,
        "created the Library's app folder"
    );
    Ok(created.id)
}

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
        let cause = FailedResponse::read(response, READ_OPERATION)
            .await
            .into_error(folder_id);
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

/// The one failure the create reports, whatever step it happened at.
fn failed(name: &str, cause: AppFolderDefect) -> Error {
    Error::AppFolderNotCreated {
        name: name.to_owned(),
        cause,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use coffret_logging::testing::CapturedLogs;
    use tracing::Level;

    use crate::http::{StubAnswer, StubTransport};
    use crate::test_support::CountingTokens;

    /// A Library whose name carries every kind of hex digit.
    fn library() -> LibraryId {
        LibraryId::from_bytes([0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef])
    }

    /// The name that Library's folder must be created under (spec: FM-18).
    const FOLDER_NAME: &str = "coffret-0123456789abcdef";

    /// What the created folder is asked for, and read back from.
    fn created() -> StubAnswer {
        StubAnswer::json(200, r#"{"id":"folder-1"}"#)
    }

    /// The metadata one call carried.
    fn metadata_of(transport: &StubTransport, index: usize) -> serde_json::Value {
        serde_json::from_slice(&transport.request(index).body)
            .expect("the create must carry a JSON body")
    }

    #[tokio::test]
    async fn a_folder_is_created_under_the_parent_it_is_given() {
        let transport = StubTransport::new([created()]);
        let tokens = CountingTokens::new();

        let id = create_app_folder(transport.clone(), tokens, "parent-1", library())
            .await
            .expect("Drive answered with the folder it created");

        // The id Drive minted is what a store is built on, so it is the answer
        // rather than anything derived from the name asked for.
        assert_eq!(id, "folder-1");

        let request = transport.request(0);
        assert!(matches!(request.method, Method::Post));
        assert!(
            request.url.contains("fields=id"),
            "the create asks for the id: {}",
            request.url
        );
        assert_eq!(
            metadata_of(&transport, 0),
            json!({
                "name": FOLDER_NAME,
                "mimeType": "application/vnd.google-apps.folder",
                "parents": ["parent-1"],
            })
        );
    }

    // Every folder goes in a parent the caller named. There is no way to ask
    // for the top of My Drive, which is never where an application's folder
    // belongs.
    #[tokio::test]
    async fn a_folder_always_names_the_parent_it_goes_in() {
        let transport = StubTransport::new([created()]);
        let tokens = CountingTokens::new();

        create_app_folder(transport.clone(), tokens, "parent-1", library())
            .await
            .expect("Drive answered with the folder it created");

        let metadata = metadata_of(&transport, 0);
        assert_eq!(metadata["name"], FOLDER_NAME);
        assert_eq!(metadata["parents"], json!(["parent-1"]));
    }

    #[tokio::test]
    async fn a_refusal_names_folder_creation_as_the_step_that_failed() {
        let transport = StubTransport::new([StubAnswer::json(
            403,
            r#"{"error":{"message":"No access.","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
        )]);
        let tokens = CountingTokens::new();

        let error = create_app_folder(transport, tokens, "parent-1", library())
            .await
            .expect_err("a refused create cannot report a folder");

        match &error {
            Error::AppFolderNotCreated { name, cause } => {
                assert_eq!(name, FOLDER_NAME);
                assert!(matches!(
                    cause,
                    AppFolderDefect::Call(coffret_usecase::Error::PermissionDenied { .. })
                ));
            }
            other => panic!("expected a folder that was not created, got {other:?}"),
        }
        // The folder it was about is in the message, for whoever reads it.
        assert!(error.to_string().contains(FOLDER_NAME), "{error}");
    }

    #[tokio::test]
    async fn a_parent_that_is_gone_is_what_a_refusal_reports_as_missing() {
        let transport = StubTransport::new([StubAnswer::json(
            404,
            r#"{"error":{"message":"File not found: parent-1.","errors":[{"reason":"notFound"}]}}"#,
        )]);
        let tokens = CountingTokens::new();

        let error = create_app_folder(transport, tokens, "parent-1", library())
            .await
            .expect_err("a create into a folder that is gone cannot report one");

        let Error::AppFolderNotCreated {
            cause: AppFolderDefect::Call(coffret_usecase::Error::NotFound { object }),
            ..
        } = &error
        else {
            panic!("expected the parent to be reported as missing, got {error:?}");
        };
        assert_eq!(object, "parent-1");
    }

    #[tokio::test]
    async fn an_answer_without_an_id_is_not_read_as_a_folder() {
        let transport = StubTransport::new([StubAnswer::json(200, r#"{"kind":"drive#file"}"#)]);
        let tokens = CountingTokens::new();

        let error = create_app_folder(transport, tokens, "parent-1", library())
            .await
            .expect_err("an answer naming no folder cannot report one");

        assert!(
            matches!(
                error,
                Error::AppFolderNotCreated {
                    cause: AppFolderDefect::Answer(_),
                    ..
                }
            ),
            "expected an unreadable answer, got {error:?}"
        );
    }

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
            cause: AppFolderDefect::Call(coffret_usecase::Error::NotFound { object }),
        } = &error
        else {
            panic!("expected the folder to be reported as missing, got {error:?}");
        };
        assert_eq!(folder_id, "folder-1");
        assert_eq!(object, "folder-1");
    }
}
