use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::state::ServerState;

mod browse_query;
pub use browse_query::BrowseQuery;

/// The folders directly inside one folder on this device.
#[derive(Serialize)]
pub struct BrowsedDto {
    /// The folder this is a listing of, as a whole path with no symbolic link
    /// left in it — which is what a mapping of it would record, so the page
    /// offers to map the folder this names rather than the text it sent.
    path: String,
    /// The folder above it, and `null` at the root of the filesystem.
    parent: Option<String>,
    /// The folders inside it, sorted by name.
    folders: Vec<BrowsedFolderDto>,
}

#[derive(Serialize)]
struct BrowsedFolderDto {
    name: String,
    path: String,
}

/// `GET /api/browse?path=<absolute>`
///
/// The folders on this device a person may choose from to map part of the
/// Library to (spec: EP-9). A page cannot be handed a real path on the device —
/// a browser's own folder picker hands it files, never where they are — so the
/// choosing goes through here: this lists, the page browses, and
/// [`map`](fn@super::map) records.
///
/// Keyed like every route (spec: LA-2), and refused while the Library is locked
/// although nothing here needs its key: the one gesture this serves is mapping,
/// which is offered only over a listing, and a locked server refuses listings.
///
/// Folders only, never files: nothing a person is choosing between here is a
/// file. A name starting with a dot is left out, as a file manager leaves it
/// out, and so is a name that is not text, which the page could neither show
/// nor send back. A symbolic link is left out too, whatever it points at: the
/// listing reads what the path names and walks no link out of it — a folder
/// reached only through one can still be typed. Nothing below the one folder is
/// read.
pub async fn browse(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<BrowseQuery>,
) -> Result<Json<BrowsedDto>, ApiError> {
    state.unlocked()?;
    let asked = match query.path.as_deref() {
        None | Some("") => home()?,
        Some(text) => absolute(text)?,
    };
    // A folder can be slow to read — a network mount, a disk spinning up — and
    // none of that is for the runtime's own threads to wait on.
    tokio::task::spawn_blocking(move || listed(&asked))
        .await
        .map_err(|_| ApiError::server("BrowseNotJoined".to_owned()))?
        .map(Json)
}

/// The text a caller sent, as a whole path on this device, or the refusal of a
/// relative one.
pub(crate) fn absolute(text: &str) -> Result<PathBuf, ApiError> {
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err(ApiError::not_absolute(text));
    }
    Ok(path)
}

/// The account's home directory, which is where a person choosing a folder of
/// their own starts.
fn home() -> Result<PathBuf, ApiError> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or_else(ApiError::no_home)
}

fn listed(asked: &Path) -> Result<BrowsedDto, ApiError> {
    // Resolved before it is read, so that `..`, a trailing separator, and a link
    // the path itself goes through all come back as the one folder they name —
    // the form the mapping of it would record.
    let path =
        fs::canonicalize(asked).map_err(|cause| ApiError::folder_not_listed(asked, &cause))?;
    if !path.is_dir() {
        return Err(ApiError::not_a_folder(asked));
    }
    let text = |path: &Path| {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| ApiError::path_not_text(path))
    };

    let mut folders = Vec::new();
    for entry in fs::read_dir(&path).map_err(|cause| ApiError::folder_not_listed(&path, &cause))? {
        let entry = entry.map_err(|cause| ApiError::folder_not_listed(&path, &cause))?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') {
            continue;
        }
        // The entry's own type, which does not follow a link: a link to a
        // folder is a link, and is left out.
        let is_folder = entry
            .file_type()
            .map_err(|cause| ApiError::folder_not_listed(&path, &cause))?
            .is_dir();
        if !is_folder {
            continue;
        }
        let Ok(child) = text(&entry.path()) else {
            continue;
        };
        folders.push(BrowsedFolderDto { name, path: child });
    }
    folders.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(BrowsedDto {
        parent: path.parent().map(text).transpose()?,
        path: text(&path)?,
        folders,
    })
}
