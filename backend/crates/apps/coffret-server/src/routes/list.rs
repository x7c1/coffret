use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use coffret_device::{AddedFile, ChildFolder, ContainerKind, EntryState, FileRow};
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
    /// Whether a folder on this device stands for this part of the Library
    /// (spec: EP-9).
    ///
    /// Here rather than left to the file route's refusal, because the mappings
    /// answer it before anything is clicked: a browser told `false` says "this
    /// folder is not on this device" over the rows it is already showing,
    /// instead of letting a reader ask for a file, wait out a round trip to
    /// Storage, and be declined. It says nothing about what is on disk — that
    /// is each row's `state`.
    mapped: bool,
    folders: Vec<FolderDto>,
    files: Vec<FileDto>,
}

#[derive(Serialize)]
struct FolderDto {
    name: String,
    path: String,
    /// Whether a folder on this device stands for it, as `mapped` above means
    /// it. In the row because mappings are made at the top level, so the
    /// children of the Library root are where two siblings can differ.
    mapped: bool,
}

#[derive(Serialize)]
struct FileDto {
    name: String,
    path: String,
    size: u64,
    /// The modification time in UTC, and `null` for the count of seconds no
    /// calendar reaches.
    ///
    /// The Entry's own (spec: FM-9) for a row the Library holds, and the local
    /// file's for an `uploading` one — which is the only time this device has to
    /// give about a file the Library has never seen.
    mtime: Option<String>,
    /// `present` or `remote` (spec: EP-10), or `uploading`.
    ///
    /// The first two are the states this device knows about an Entry. What the
    /// explorer shows while a fetch is running, and what it shows when one
    /// failed, are states of a request the browser made — nothing on this device
    /// changes between asking for an Entry and getting it — so they are the
    /// browser's to hold and not this route's to invent.
    ///
    /// `uploading` is neither: it is a file standing in the mapped folder that
    /// the Library holds no Entry for, so it is not a state *of* an Entry at all.
    /// It is here because it is the honest answer about the folder — the file is
    /// there, somebody put it there, and the Library does not have it yet.
    state: &'static str,
    /// `one-file` or `pack` (spec: PK-15), and `null` for an `uploading` row.
    ///
    /// Null rather than a guess, because there is no Container: nothing has been
    /// committed for this file, and what it will live in is the next sync's
    /// answer. What reads this field decides from it whether an Entry can be
    /// replaced one file at a time, and a row with no Entry has nothing to
    /// replace.
    container: Option<&'static str>,
    openable: bool,
    content_type: &'static str,
}

/// `GET /api/list?path=<folder>`
///
/// Two answers about one folder, merged into one list of rows: what the Library
/// holds there, and what is standing in the mapped folder that the Library does
/// not hold. The second is what makes a file appear the moment it is dropped —
/// nothing has been committed for it, so no catalog row exists to list, and the
/// folder itself is the only thing that knows it is there.
pub async fn list(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<Json<ListingDto>, ApiError> {
    let library = state.unlocked()?;
    let folder = query.folder()?;
    let listing = library.list(folder.as_ref()).await?;
    let added = library.added_locally(folder.as_ref()).await?;

    Ok(Json(ListingDto {
        path: listing
            .path
            .as_ref()
            .map(|path| path.as_str().to_owned())
            .unwrap_or_default(),
        mapped: listing.mapped,
        folders: listing.folders.iter().map(folder_dto).collect(),
        files: merged(&listing.files, &added),
    }))
}

/// The Library's rows and the folder's own, in one list in EP-3 order.
///
/// Both come sorted by Entry Path — the byte order of the canonical paths, which
/// is the one order every device agrees on — so this is a merge and never a
/// re-sort: a dropped file appears where its name puts it rather than at the
/// bottom, and the row it becomes after the sync is in the same place.
///
/// The two cannot name one path. A file the Library holds an Entry for is not
/// among the added ones, which is exactly what
/// [`added_locally`](coffret_device::OpenLibrary::added_locally) leaves out.
fn merged(files: &[FileRow], added: &[AddedFile]) -> Vec<FileDto> {
    let mut rows = Vec::with_capacity(files.len() + added.len());
    let mut held = files.iter().peekable();
    let mut waiting = added.iter().peekable();
    loop {
        let next = match (held.peek(), waiting.peek()) {
            (Some(file), Some(one)) if file.path <= one.path => Held(file),
            (Some(_), Some(one)) => Added(one),
            (Some(file), None) => Held(file),
            (None, Some(one)) => Added(one),
            (None, None) => return rows,
        };
        rows.push(match next {
            Held(file) => {
                held.next();
                file_dto(file)
            }
            Added(one) => {
                waiting.next();
                added_dto(one)
            }
        });
    }
}

/// Which of the two lists the next row comes from.
enum Next<'a> {
    Held(&'a FileRow),
    Added(&'a AddedFile),
}
use Next::{Added, Held};

fn folder_dto(folder: &ChildFolder) -> FolderDto {
    FolderDto {
        name: folder.name.clone(),
        path: folder.path.as_str().to_owned(),
        mapped: folder.mapped,
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
        container: Some(match file.container {
            ContainerKind::OneFile => "one-file",
            ContainerKind::Pack => "pack",
        }),
        openable: media.openable,
        content_type: media.content_type,
    }
}

/// One file standing in the mapped folder that the Library does not hold.
///
/// The same row shape as an Entry's, because to whoever is looking at the folder
/// it is the same kind of thing: a file with a name, a size and a time, which
/// opens in the reader like any other. What it has instead of a state and a
/// Container is the one fact that distinguishes it — the Library does not have
/// this yet.
fn added_dto(file: &AddedFile) -> FileDto {
    let media = classify(&file.path);
    FileDto {
        name: file.name.clone(),
        path: file.path.as_str().to_owned(),
        size: file.size,
        mtime: iso8601(file.mtime),
        state: "uploading",
        container: None,
        openable: media.openable,
        content_type: media.content_type,
    }
}
