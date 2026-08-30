//! The Library's app folder on Drive, before there is a store to open.
//!
//! Everything else in this crate works inside one Library's folder: the
//! `ObjectStore` port is scoped to a Library, and [`GoogleDrive`] is built from
//! the folder id it is to work in. This module is the stage before that — the
//! folder does not exist yet, so there is nothing for a store to be built on —
//! which is why it takes the transport and the token source directly rather
//! than hanging off either the port or the store.
//!
//! [`GoogleDrive`]: crate::GoogleDrive

use std::sync::Arc;

use coffret_model::LibraryId;
use serde_json::json;
use tracing::info;

use crate::api::{authorization, DriveApi, Endpoints, FailedResponse, FileResource};
use crate::error::{AppFolderDefect, Error, Result};
use crate::http::{HttpRequest, HttpTransport, Method};
use crate::oauth::AccessTokens;

/// What Drive calls a folder.
const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";

/// What this call is recorded and reported as.
///
/// The same name on the event that says the folder was created and on the one
/// that says it was not, so a reader asking what this operation did sees both.
const OPERATION: &str = "create_app_folder";

/// Creates the folder one Library's objects will live in, and reports its id.
///
/// The folder is named `coffret-<library id>` (spec: FM-18), which is what a
/// device recovering with only a Recovery Code enumerates for. `parent` is the
/// folder it goes in; `None` puts it at the top of My Drive. Naming no parent
/// is how Drive is asked for that placement — `root` is an alias for a folder
/// this application did not create, so asking for it as a parent asks for
/// access the `drive.file` grant does not cover.
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
    parent: Option<&str>,
    library: LibraryId,
) -> Result<String> {
    let name = library.app_folder_name();
    let mut metadata = json!({
        "name": name,
        "mimeType": FOLDER_MIME_TYPE,
    });
    if let Some(parent) = parent {
        metadata["parents"] = json!([parent]);
    }

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
        // gone" into "the Library's folder is gone". Where no parent was named
        // there is nothing but My Drive it could be about.
        let enclosing = parent.unwrap_or("My Drive");
        let cause = FailedResponse::read(response, OPERATION)
            .await
            .into_error(enclosing);
        return Err(failed(&name, AppFolderDefect::Call(cause)));
    }

    let body = response
        .into_body()
        .into_bytes()
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

/// The one failure this module reports, whatever step it happened at.
fn failed(name: &str, cause: AppFolderDefect) -> Error {
    Error::AppFolderNotCreated {
        name: name.to_owned(),
        cause,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let id = create_app_folder(transport.clone(), tokens, Some("parent-1"), library())
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

    #[tokio::test]
    async fn a_folder_with_no_parent_names_none() {
        let transport = StubTransport::new([created()]);
        let tokens = CountingTokens::new();

        create_app_folder(transport.clone(), tokens, None, library())
            .await
            .expect("Drive answered with the folder it created");

        let metadata = metadata_of(&transport, 0);
        assert_eq!(metadata["name"], FOLDER_NAME);
        assert!(
            metadata.get("parents").is_none(),
            "My Drive is asked for by omitting the field: {metadata}"
        );
    }

    #[tokio::test]
    async fn a_refusal_names_folder_creation_as_the_step_that_failed() {
        let transport = StubTransport::new([StubAnswer::json(
            403,
            r#"{"error":{"message":"No access.","errors":[{"reason":"insufficientFilePermissions"}]}}"#,
        )]);
        let tokens = CountingTokens::new();

        let error = create_app_folder(transport, tokens, Some("parent-1"), library())
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

        let error = create_app_folder(transport, tokens, Some("parent-1"), library())
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

        let error = create_app_folder(transport, tokens, None, library())
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
}
