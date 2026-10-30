use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use coffret_device::FilesBeside;
use serde::{Deserialize, Serialize};

use crate::api_error::ApiError;
use crate::entry_query::PathQuery;
use crate::state::ServerState;

/// The `?folder=` the route may be asked with beside `?path=`: whether the row
/// is the folder at that path rather than the file, since the Library admits
/// both at one path.
///
/// A query of its own for the reason the file route's `?peek=` is one: it is a
/// way of asking this route alone.
#[derive(Debug, Deserialize)]
pub struct RowQuery {
    #[serde(default)]
    folder: bool,
}

/// The files either side of one row, in reading order (spec: LA-32).
#[derive(Serialize)]
pub struct BesideDto {
    /// The Entry Path of the file before the row, `null` at the start of the
    /// Library.
    previous: Option<String>,
    /// The file after it, `null` at the end.
    next: Option<String>,
}

impl From<FilesBeside> for BesideDto {
    fn from(beside: FilesBeside) -> Self {
        Self {
            previous: beside.previous.map(|path| path.as_str().to_owned()),
            next: beside.next.map(|path| path.as_str().to_owned()),
        }
    }
}

/// `GET /api/beside?path=<row>[&folder=true]`
///
/// The file a reader steps back onto and the one it reads on into from the
/// row at `path`, across the whole Library (spec: LA-32). The explorer's tree
/// holds the listings of the folders it has opened and no others, so it cannot
/// know which file stands first in a folder it has not opened; this is how a
/// reader at the end of one folder finds the first file of the next.
///
/// Out of the catalog alone, as a listing's answer is, but unlike a listing it
/// sets no read of Containers' fronts going: it reads nothing of Storage and
/// arms nothing, and a path the Library holds nothing at is answered for the
/// place it names rather than refused.
pub async fn beside(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
    Query(row): Query<RowQuery>,
) -> Result<Json<BesideDto>, ApiError> {
    let library = state.unlocked()?;
    let path = query.entry()?;
    Ok(Json(library.files_beside(&path, row.folder).await?.into()))
}
