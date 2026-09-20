//! Lists and trashes the app folders coffret left under one folder on Drive.
//!
//! The manual Drive targets each keep a Library, and a Library's objects live
//! in a `coffret-<library id>` folder they create once and reuse for ever
//! after. Nothing removes those folders — no flow discards a Library — and
//! their names say nothing about when they were made or which target made
//! them, so an account that has run the targets for a while holds folders
//! whose only difference is that some of them are still pointed at by a
//! Library on this device and some are not. This is the tool that shows them
//! and takes the leftovers away; `scripts/drive-it-reset.sh` is what drives
//! it, and what knows which of them a Library here still points at.
//!
//! ```text
//! COFFRET_DRIVE_CLIENT_ID      the OAuth client the grant belongs to
//! COFFRET_DRIVE_CLIENT_SECRET  optional, for a client registered with one
//! COFFRET_DRIVE_TOKEN_CACHE    where that grant was cached
//! COFFRET_MASTER_KEY           the Master Key the cache is sealed under,
//!                              base64 of 32 bytes
//!
//! cargo run -p google-drive-store --example app_folders -- list <parent id>
//! cargo run -p google-drive-store --example app_folders -- trash <id>...
//! ```
//!
//! `list` prints one line per live child folder of the parent whose name
//! begins `coffret-` — id, name, `createdTime`, separated by tabs, oldest
//! first. `trash` puts each folder it is given in Drive's trash rather than
//! deleting it, so a folder that turns out to have mattered is recoverable for
//! a while. Neither subcommand takes `root`: it is an alias for the top of My
//! Drive rather than an id either of them may name, and both refuse it before
//! anything is sent.
//!
//! A grant of this example's own sees those folders even though the CLI
//! created them: a `drive.file` grant reaches what *the OAuth client* created,
//! on any device and under any of that client's grants, rather than what the
//! one process that made them can reach. So this tool authorizes once, as the
//! same client the targets use, and finds the Libraries they made — while
//! still reaching nothing else in the account.
//!
//! What Drive answered goes to the log file the state directory holds, the way
//! the `authorize` example's answers do, since a refusal is
//! the only thing that says why a listing or a removal did not happen.
//!
//! This is test support and is not shipped: it lives with the gateway it calls
//! because it is that gateway's own pieces — `DriveApi`, `Endpoints`, the
//! token cache — rather than a second Drive client.

use std::fmt::Display;
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use coffret_format::{Purpose, PurposeKey};
use coffret_logging::redact::PrivateValues;
use coffret_logging::{install, LogSettings};
use coffret_model::MasterKey;
use coffret_usecase::Missing;
use google_drive_store::http::{HttpRequest, HttpTransport, Method};
use google_drive_store::{
    authorization, live_files_query, AccessTokens, ClientCredentials, DriveApi, Endpoints,
    FailedResponse, OAuthTokens, ReqwestTransport, TokenCache,
};
use serde::Deserialize;
use serde_json::json;

/// What Drive calls a folder.
const FOLDER_MIME_TYPE: &str = "application/vnd.google-apps.folder";

/// The beginning of every folder name coffret creates (spec: FM-18).
///
/// Matched here rather than in the query: Drive's `contains` is a prefix match
/// for a name, but only loosely so — it matches a word of the name as well —
/// and what is being asked is whether the folder is one coffret made, which is
/// a question about the whole of the name.
const APP_FOLDER_PREFIX: &str = "coffret-";

/// The id no run may name, as a parent to list under or a folder to trash.
///
/// `root` is an alias for a folder this application did not create rather than
/// an id it may name, and the placement it stands for — the top of My Drive —
/// is not one a test's folders belong at. The Drive test support refuses it for
/// the same reason (`tests/support/mod.rs`).
const MY_DRIVE: &str = "root";

/// How much of one page of the listing this example will take into memory.
///
/// The gateway's own ceiling for a `files.list` page, which is not exported:
/// a page holds as many folders as the page size, each of them a handful of
/// named fields, and the headroom above that is the point past which the
/// answer is not a listing at all.
const MAX_LISTING_PAGE_LEN: u64 = 16 * 1024 * 1024;

/// How many folders one page asks for.
const PAGE_SIZE: u32 = 100;

/// One folder as this example asks Drive to describe it.
///
/// `createdTime` is the field that makes the listing worth reading — it is the
/// only thing on the folder that says when the run that made it happened — and
/// it is not among the fields the gateway's own listing asks for, so the shape
/// is declared here rather than borrowed from `FileResource`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Folder {
    /// The id Drive minted, which is what a removal names.
    id: String,
    /// What the folder is called.
    name: String,
    /// When Drive created it, RFC 3339.
    created_time: String,
}

/// One page of the listing.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FolderList {
    /// The folders on this page, absent from the answer when there are none.
    #[serde(default)]
    files: Vec<Folder>,
    /// What to ask for the next page with, absent on the last one.
    next_page_token: Option<String>,
}

#[tokio::main]
async fn main() {
    start_logging();

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        usage("say which of list or trash to run");
    };

    match (command.as_str(), rest) {
        ("list", [parent]) => {
            // Before the grant is opened and before anything is sent: a parent
            // this tool will not work under is an answer it can give on its
            // own.
            refuse_my_drive_among(std::slice::from_ref(parent));
            let api = api();
            for folder in list(&api, parent).await {
                println!("{}\t{}\t{}", folder.id, folder.name, folder.created_time);
            }
        }
        ("list", _) => usage("list takes one folder id: the parent to look under"),
        ("trash", []) => usage("trash takes the ids of the folders to trash"),
        ("trash", ids) => {
            // Before the grant is opened and before anything is sent, as in
            // `list`.
            refuse_my_drive_among(ids);
            let api = api();
            for id in ids {
                trash(&api, id).await;
                println!("trashed {id}");
            }
        }
        (other, _) => usage(format!("{other:?} is neither list nor trash")),
    }
}

/// Stops where any of `ids` is the top of My Drive rather than a folder.
///
/// All or nothing: one `root` among several ids refuses the whole call, so a
/// typo does not trash the rest of the list first.
fn refuse_my_drive_among(ids: &[String]) {
    if ids.iter().any(|id| id == MY_DRIVE) {
        fail(format!(
            "{MY_DRIVE:?} is not a folder id: it is an alias for the top of My Drive, which is \
             not where coffret's folders are. Name a folder by the id in its address: the one \
             the Libraries were created in to list under, or one of the `coffret-` folders it \
             holds to trash."
        ));
    }
}

/// Every live `coffret-` folder directly under `parent`, oldest first.
async fn list(api: &DriveApi, parent: &str) -> Vec<Folder> {
    // Folders alone, under this parent alone, and none of the trashed ones: a
    // reset that listed what it had already trashed would trash it again on
    // every run, and a listing that reached past the parent would be offering
    // to remove folders nobody pointed this tool at.
    let query = format!(
        "{} and mimeType = '{FOLDER_MIME_TYPE}'",
        live_files_query(parent)
    );

    let mut folders = Vec::new();
    let mut page: Option<String> = None;
    loop {
        let url = {
            let mut parameters = url::form_urlencoded::Serializer::new(String::new());
            let page_size = PAGE_SIZE.to_string();
            parameters.extend_pairs([
                ("q", query.as_str()),
                ("fields", "nextPageToken,files(id,name,createdTime)"),
                ("pageSize", page_size.as_str()),
                // Oldest first, which is the order the question is asked in:
                // the folder a run left behind months ago is the one being
                // looked for.
                ("orderBy", "createdTime"),
            ]);
            if let Some(token) = &page {
                parameters.append_pair("pageToken", token);
            }
            format!("{}?{}", api.endpoints().files(), parameters.finish())
        };

        let response = api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Get, &url)
                    .with_header(header, value)
                    .within(MAX_LISTING_PAGE_LEN)
            })
            .await
            .unwrap_or_else(|error| fail(format!("could not list the folders: {error}")));

        if !response.is_success() {
            // The parent is somebody's own Drive — a folder they chose — and
            // Drive quotes what it was asked for, so it goes out of the record
            // of the refusal (spec: EL-5). What can be missing here is that
            // folder and nothing of coffret's.
            let error = FailedResponse::read(response, "list_app_folders", &private(parent))
                .await
                .into_error(Missing::Location);
            fail(format!("could not list the folders: {error}"));
        }

        let body = response
            .into_body()
            .into_bytes_within(MAX_LISTING_PAGE_LEN)
            .await
            .unwrap_or_else(|error| fail(format!("could not read the listing: {error}")));
        let listing: FolderList = serde_json::from_slice(&body)
            .unwrap_or_else(|error| fail(format!("could not read the listing: {error}")));

        folders.extend(
            listing
                .files
                .into_iter()
                .filter(|folder| folder.name.starts_with(APP_FOLDER_PREFIX)),
        );

        match listing.next_page_token {
            Some(token) => page = Some(token),
            None => return folders,
        }
    }
}

/// Puts one folder in Drive's trash.
///
/// Trashed and not purged: what a run left behind may turn out to be a Library
/// somebody wanted, and the trash is where that is recoverable for a while
/// rather than gone. Emptying it is the account owner's own.
async fn trash(api: &DriveApi, id: &str) {
    let url = format!("{}?fields=id", api.endpoints().file(id));
    let trashed = json!({ "trashed": true });

    let response = api
        .send(|token| {
            let (header, value) = authorization(token);
            HttpRequest::new(Method::Patch, &url)
                .with_header(header, value)
                .with_json(&trashed)
        })
        .await
        .unwrap_or_else(|error| fail(format!("could not trash {id}: {error}")));

    if !response.is_success() {
        // Nothing private: the folder is one coffret created, named after the
        // Library it holds, and the id is Drive's own (spec: EL-5).
        let error = FailedResponse::read(response, "trash_app_folder", &PrivateValues::none())
            .await
            .into_object_error(id);
        fail(format!("could not trash {id}: {error}"));
    }
}

/// What Drive may echo back that is not this tool's to record.
fn private(parent: &str) -> PrivateValues {
    PrivateValues::none().with(parent)
}

/// Builds the authorized API client this example makes both of its calls with.
///
/// The grant is the one cached at `COFFRET_DRIVE_TOKEN_CACHE`, which the
/// `authorize` example wrote and sealed under the same Master Key.
fn api() -> DriveApi {
    let cache = TokenCache::new(
        require("COFFRET_DRIVE_TOKEN_CACHE"),
        Arc::new(PurposeKey::derive(&master_key(), Purpose::TokenCache)),
    );

    let mut credentials = ClientCredentials::new(require("COFFRET_DRIVE_CLIENT_ID"));
    if let Ok(secret) = std::env::var("COFFRET_DRIVE_CLIENT_SECRET") {
        credentials = credentials.with_client_secret(secret);
    }

    let transport: Arc<dyn HttpTransport> = Arc::new(
        ReqwestTransport::with_default_client().expect("an HTTP client must be buildable"),
    );
    let tokens: Arc<dyn AccessTokens> =
        Arc::new(OAuthTokens::new(transport.clone(), credentials, cache));
    DriveApi::new(transport, tokens, Endpoints::default())
}

/// Points this run's events at the log file.
///
/// An example is an application, and an application is what installs a
/// subscriber: the library crates it drives only emit.
fn start_logging() {
    let settings = LogSettings::from_env().unwrap_or_else(|error| fail(error));
    match install(&settings) {
        // On standard error, because standard output is the listing and a
        // script reads it.
        Ok(path) => eprintln!("Logging this run to {}.", path.display()),
        Err(error) => fail(format!("could not start logging: {error}")),
    }
}

/// Reads the Master Key the token cache is sealed under.
fn master_key() -> MasterKey {
    let encoded = require("COFFRET_MASTER_KEY");
    let bytes = STANDARD
        .decode(encoded.trim())
        .unwrap_or_else(|error| fail(format!("COFFRET_MASTER_KEY must be base64: {error}")));

    let bytes: [u8; MasterKey::BYTE_LEN] = bytes.as_slice().try_into().unwrap_or_else(|_| {
        fail(format!(
            "COFFRET_MASTER_KEY must decode to {} bytes, not {}",
            MasterKey::BYTE_LEN,
            bytes.len()
        ))
    });
    MasterKey::from_bytes(bytes)
}

/// Reads a variable neither subcommand can run without.
fn require(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| fail(format!("{name} must be set")))
}

/// Says how this example is called, and stops.
fn usage(complaint: impl Display) -> ! {
    fail(format!(
        "{complaint}\n\
         \n\
         app_folders list <parent id>   the coffret- folders under that folder\n\
         app_folders trash <id>...      put each of those folders in the trash"
    ))
}

/// Says what went wrong, and stops.
fn fail(message: impl Display) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
