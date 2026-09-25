//! Every refusal this server can send, written out as the wire carries it, for
//! the explorer's own cases to read back through its types.
//!
//! The two sides are built by different toolchains, so what holds them to one
//! contract is a committed file between them, in the shape the finding names
//! already travel in: this side writes each refusal the way a route answers
//! with it — through `IntoResponse`, status and body — and the explorer's
//! `contract.test.ts` reads every one of them through `refusalOf` and the unions
//! a screen branches on. A kind, a reason or a finding name this side grows
//! fails here until the file is brought along, and fails there until the
//! explorer has a word for it.
//!
//! One of each combination rather than one of each constructor: what the
//! explorer branches on is the triple of `error`, `reason` and `surfaced`, and a
//! combination the server sends that the explorer has never been shown is the
//! gap this is for. `written` rides along once, on the refusal that carries it.

use std::path::{Path, PathBuf};

use axum::response::IntoResponse;
use coffret_device::{CommitError, Error, FetchError, RefusedRoot, RootRefused, Surfaced};
use coffret_model::{ContainerId, Generation};

use super::ApiError;
use crate::entry_paths::entry_path;

/// Where the explorer reads the refusals from, relative to this crate.
const REFUSALS: &str = "../../../../frontend/packages/gateway/api/src/contract/refusals.json";

/// What rewrites the committed files instead of comparing against them.
///
/// For a change that means to alter what goes on the wire: the diff of the file
/// is then the change's statement of what the explorer will now receive.
const WRITE: &str = "COFFRET_WRITE_CONTRACT";

/// Every refusal the routes can answer with, one per combination a browser
/// branches on, in the order the explorer's `RefusalKind` names the kinds.
fn every_refusal() -> Vec<ApiError> {
    let fetch = |cause: FetchError| {
        ApiError::from(Error::Fetch {
            cause: Box::new(cause),
        })
    };
    let path = || entry_path("albums/spring.jpg");
    let container_id = ContainerId::from_bytes([0x11; ContainerId::BYTE_LEN]);

    let mut refusals = vec![
        ApiError::bad_path("an Entry Path is never empty"),
        ApiError::whole_drop_too_large(),
        ApiError::unauthorized("this server answers only the explorer it was started for"),
        ApiError::no_such_entry(),
        ApiError::no_such_route(),
        ApiError::no_such_method(),
        ApiError::no_folder_here(),
        fetch(FetchError::UnmaterializablePath {
            path: path(),
            stopped_at: None,
        }),
        fetch(FetchError::ReservedComponent {
            path: entry_path("albums/.coffret/root"),
            component: ".coffret".to_owned(),
        }),
        fetch(FetchError::RefusedRoot(RefusedRoot {
            prefix: Some(entry_path("albums")),
            local_root: PathBuf::from("/mnt/copied"),
            reason: RootRefused::MarkerMismatch,
        })),
        ApiError::pack_resident(),
    ];
    // Every finding a declined fetch can name, which is `locked` beside
    // `KeyLost` and `surfaced` beside every other.
    for surfaced in [
        Surfaced::ForeignFile { path: path() },
        Surfaced::LocallyChanged { path: path() },
        Surfaced::WitnessedDeletion { path: path() },
        Surfaced::UnreachablePlace {
            path: path(),
            stopped_at: PathBuf::from("/home/someone/albums"),
        },
        Surfaced::KeyLost {
            path: path(),
            container_id,
        },
        Surfaced::ReservedComponent {
            path: entry_path("albums/.coffret/root"),
        },
    ] {
        refusals.push(ApiError::declined(&surfaced));
    }
    refusals.extend([
        ApiError::from(Error::Fetch {
            cause: Box::new(FetchError::Commit(CommitError::EpochActivated {
                generation: Generation::new(41).expect("a small generation is one"),
            })),
        }),
        ApiError::locked(),
        fetch(FetchError::Storage(
            coffret_usecase::Error::Unauthenticated {
                detail: "the grant has run out".to_owned(),
                source: None,
            },
        )),
        fetch(FetchError::ContentMismatch {
            container_id,
            path: path(),
        }),
        ApiError::server("Device::Index".to_owned()),
        // The one refusal that says what landed before it: a drop stopped
        // part way, whose files are in the folder already.
        ApiError::no_room().having_written(vec!["albums/one.jpg".to_owned()]),
    ]);
    refusals
}

/// One refusal as the wire carries it: the status, and the body a route sends.
async fn on_the_wire(refusal: ApiError) -> serde_json::Value {
    let response = refusal.into_response();
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a refusal's body is in memory");
    let body: serde_json::Value = serde_json::from_slice(&body).expect("a refusal's body is JSON");
    serde_json::json!({ "status": status, "body": body })
}

/// `value` with every object's fields in the order of their names.
///
/// Rendered that way whatever order the value was built in: a workspace build
/// can turn on `serde_json`'s `preserve_order` for every crate at once, and a
/// file whose field order followed the build would change with it.
fn canonical(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(fields) => {
            let mut names: Vec<&String> = fields.keys().collect();
            names.sort();
            serde_json::Value::Object(
                names
                    .into_iter()
                    .map(|name| (name.clone(), canonical(&fields[name])))
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(canonical).collect())
        }
        other => other.clone(),
    }
}

/// Compares `written` with the file at `relative`, or writes it there where
/// [`WRITE`] is set.
pub(crate) fn held_to(relative: &str, written: &serde_json::Value) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(&canonical(written)).expect("a JSON value renders")
    );
    if std::env::var_os(WRITE).is_some() {
        std::fs::write(&path, rendered)
            .unwrap_or_else(|cause| panic!("{} must be writable: {cause}", path.display()));
        return;
    }
    let held = std::fs::read_to_string(&path)
        .unwrap_or_else(|cause| panic!("{} must be readable: {cause}", path.display()));
    assert!(
        held == rendered,
        "{} is not what this server sends. If the change means to alter the wire, run the \
         case again with {WRITE}=1 and read the file's diff as what the explorer will now \
         receive — then make its `contract.test.ts` pass over it. What this server sends \
         now:\n{rendered}",
        path.display(),
    );
}

// The explorer's half of the contract reads this file; this is what holds the
// file to the server. A refusal whose kind, reason or finding name changes on
// this side fails here until the file follows.
#[tokio::test]
async fn the_refusals_the_explorer_reads_are_the_ones_this_server_sends() {
    let mut written = Vec::new();
    for refusal in every_refusal() {
        written.push(on_the_wire(refusal).await);
    }
    held_to(REFUSALS, &serde_json::Value::Array(written));
}
