use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use coffret_device::{ChildFolder, ContainerKind, EntryState, FileRow};
use serde::Serialize;

use crate::api_error::ApiError;
use crate::classify::classify;
use crate::entry_query::PathQuery;
use crate::state::ServerState;
use crate::timestamp::iso8601;

/// What one folder holds, one level down.
#[derive(Serialize)]
pub struct ListingDto {
    /// The folder this is a listing of; the Library root is the empty string,
    /// because the root is not a path (spec: EP-2) and every other field here
    /// is one.
    path: String,
    folders: Vec<FolderDto>,
    files: Vec<FileDto>,
}

#[derive(Serialize)]
struct FolderDto {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct FileDto {
    name: String,
    path: String,
    size: u64,
    /// The Entry's own modification time in UTC, and `null` for the count of
    /// seconds no calendar reaches.
    mtime: Option<String>,
    /// `present` or `remote` (spec: EP-10).
    ///
    /// The two states this device knows. What the explorer shows while a fetch
    /// is running, and what it shows when one failed, are states of a request
    /// the browser made — nothing on this device changes between asking for an
    /// Entry and getting it — so they are the browser's to hold and not this
    /// route's to invent.
    state: &'static str,
    /// `one-file` or `pack` (spec: PK-15).
    container: &'static str,
    openable: bool,
    content_type: &'static str,
}

/// `GET /api/list?path=<folder>`
pub async fn list(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<Json<ListingDto>, ApiError> {
    let folder = query.folder()?;
    let listing = state.library.list(folder.as_ref()).await?;

    Ok(Json(ListingDto {
        path: listing
            .path
            .as_ref()
            .map(|path| path.as_str().to_owned())
            .unwrap_or_default(),
        folders: listing.folders.iter().map(folder_dto).collect(),
        files: listing.files.iter().map(file_dto).collect(),
    }))
}

fn folder_dto(folder: &ChildFolder) -> FolderDto {
    FolderDto {
        name: folder.name.clone(),
        path: folder.path.as_str().to_owned(),
    }
}

fn file_dto(file: &FileRow) -> FileDto {
    let media = classify(&file.path);
    FileDto {
        name: file.name.clone(),
        path: file.path.as_str().to_owned(),
        size: file.size,
        mtime: iso8601(file.mtime),
        state: match file.state {
            EntryState::Present => "present",
            EntryState::Remote => "remote",
        },
        container: match file.container {
            ContainerKind::OneFile => "one-file",
            ContainerKind::Pack => "pack",
        },
        openable: media.openable,
        content_type: media.content_type,
    }
}
