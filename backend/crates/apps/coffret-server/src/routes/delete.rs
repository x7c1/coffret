use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use coffret_device::DeletePreview;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::delete::{delete_target, RefusedPack, Target};
use crate::entry_query::{folder_named, shaped};
use crate::state::ServerState;

use super::work::WorkDto;

/// `POST /api/delete?path=<folder>` or `POST /api/delete?entry=<path>&entry=…`
///
/// Takes a folder with everything under it, or individual files, out of the
/// Library (spec: PK-9). It is what the explorer's "Delete…" arms once the
/// person has been shown what [`GET /api/delete`](preview) counted and has
/// said yes. One-file Containers and Packs whose every Entry is named are
/// removed; Packs that keep other files are rebuilt around them (spec: PK-10);
/// a Pack that keeps other files and whose key is lost, or that does not
/// verify when it is read back, is refused and left whole (spec: PK-10,
/// KL-17). No file on this device is touched.
///
/// What it deletes is named in the query, the same way the preview names it:
/// `?path=` for a folder, the spelling every route here names a folder with,
/// and `?entry=` once per file. A preview is a `GET`, which a browser sends
/// without a body, so the two read one shape and cannot come to name different
/// things.
///
/// It answers with the work answer as it stands the moment the deletion is
/// armed, `202`, rather than waiting for it: rebuilding a Pack reads it whole
/// and uploads its replacement, and the browser follows the rest by polling. A
/// second call while one is running queues behind it — one deletion at a
/// time — and a call naming exactly what is already running or waiting changes
/// nothing.
///
/// Refused before anything is armed exactly where the preview is refused: a
/// locked Library, the Library root as a whole, and a selection under which
/// the Library holds nothing current.
pub async fn delete(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<(StatusCode, Json<WorkDto>), ApiError> {
    let target = armable(&state, &query).await?;
    delete_target(Arc::clone(&state), target);
    Ok((StatusCode::ACCEPTED, Json(WorkDto::of(&state))))
}

/// `GET /api/delete?path=<folder>` or `GET /api/delete?entry=<path>&entry=…`
///
/// What [`POST /api/delete`](delete) of the same selection would do, counted
/// before it is asked for: how many files and bytes leave the Library, how
/// many Containers are removed outright, how many Packs are rebuilt and what
/// that reads and writes, which Packs it would be refused for and why, and
/// which named files the Library does not hold.
///
/// The count is the run's own plan (see
/// [`preview_delete`](coffret_device::OpenLibrary::preview_delete)), after the
/// run's own first step — the catalog caught up and the committed Keyring read
/// for the Packs whose key is lost — so the two cannot disagree about what is
/// removed and what is rebuilt. They can differ only where the Library moves
/// between the two calls, or where a Pack the run reads back does not verify,
/// which a preview reading nothing cannot know (spec: PK-10).
///
/// It changes nothing in the Library and arms nothing. Refused exactly where
/// the `POST` is.
pub async fn preview(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<Vec<(String, String)>>,
) -> Result<Json<PreviewDto>, ApiError> {
    let target = armable(&state, &query).await?;
    let preview = state
        .unlocked()?
        .preview_delete(&target.selection())
        .await?;
    Ok(Json(PreviewDto::of(
        &target,
        &preview,
        state.deletes.running(),
    )))
}

/// The target a query names, or the refusal that keeps one from being armed.
async fn armable(state: &ServerState, query: &[(String, String)]) -> Result<Target, ApiError> {
    let target = target_of(query)?;
    // Asked before anything else that needs the Library, so a locked one is
    // refused as locked rather than as anything about the selection.
    let library = state.unlocked()?;
    if target.is_whole_library() {
        // An absent or empty `?path=` is the Library root everywhere else on
        // these routes, and a deletion of the root's whole folder would be a
        // deletion of the whole Library — arrived at by omission.
        return Err(ApiError::bad_path(
            "it names no folder and no file, and a deletion is of files or a folder rather \
             than of the whole Library",
        ));
    }
    // Out of the catalog alone, so it costs no Storage: a deletion of nothing
    // would be a `202` for a run that commits nothing.
    if !library.deletes_anything(&target.selection()).await? {
        return Err(ApiError::no_such_entry());
    }
    Ok(target)
}

/// What a query names: `?path=` once, for a folder, and `?entry=` once per
/// file. Anything else in it is not read.
fn target_of(query: &[(String, String)]) -> Result<Target, ApiError> {
    let mut folder = None;
    let mut paths = BTreeSet::new();
    for (name, value) in query {
        match name.as_str() {
            "path" => folder = folder_named(Some(value))?,
            "entry" => {
                paths.insert(shaped(value)?);
            }
            _ => {}
        }
    }
    Ok(Target { folder, paths })
}

/// What a deletion would do, as the explorer shows it before it offers Delete.
#[derive(Debug, Serialize)]
pub struct PreviewDto {
    /// The folder it would delete, and `null` where it names files only.
    folder: Option<String>,
    /// The files it names, in Entry Path order.
    paths: Vec<String>,
    /// How many files would leave the Library.
    entries: usize,
    /// How many bytes those files come to.
    bytes: u64,
    /// How many Containers would be removed outright (spec: PK-9).
    removed: usize,
    /// How many Packs would be rebuilt around the files they keep (spec: PK-10).
    rebuilt: usize,
    /// How many bytes those rebuilds read from Storage: each Pack whole.
    rebuild_read: u64,
    /// How many bytes the rebuilt Packs would weigh on Storage.
    rebuild_written: u64,
    /// The Packs it would be refused for, and why (spec: KL-17).
    refused: Vec<RefusedPackDto>,
    /// Named files the Library holds nothing at.
    missing: Vec<String>,
    /// Whether a deletion is already running, so this one would wait its turn.
    after_current: bool,
}

impl PreviewDto {
    fn of(target: &Target, preview: &DeletePreview, after_current: bool) -> Self {
        Self {
            folder: target.folder_named(),
            paths: target.paths_named(),
            entries: preview.entries,
            bytes: preview.bytes,
            removed: preview.removed,
            rebuilt: preview.rebuilt,
            rebuild_read: preview.rebuild_read,
            rebuild_written: preview.rebuild_written,
            refused: RefusedPack::all_of(preview)
                .iter()
                .map(RefusedPackDto::of)
                .collect(),
            missing: preview
                .missing
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            after_current,
        }
    }
}

/// One Pack a deletion is, or would be, refused for, as the browser is told it.
///
/// Shared by the preview and the work answer, because the same refusal is
/// shown before the deletion and after it.
#[derive(Debug, Serialize)]
pub(super) struct RefusedPackDto {
    /// The named files that stay in the Library because their Pack does.
    spared: Vec<String>,
    /// How many other files the Pack holds.
    kept: usize,
    /// `key_lost` or `unverified`.
    reason: &'static str,
    /// The sentence to show beside the files that stayed.
    message: &'static str,
}

impl RefusedPackDto {
    pub(super) fn of(refused: &RefusedPack) -> Self {
        Self {
            spared: refused
                .spared
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            kept: refused.kept,
            reason: refused.reason,
            message: refused.message(),
        }
    }
}
