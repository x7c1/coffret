use std::io;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use coffret_device::{EntryFetch, EntryPath, EntryState};
use tokio::fs;
use tracing::info;

use crate::api_error::ApiError;
use crate::classify::classify;
use crate::entry_query::PathQuery;
use crate::state::ServerState;

/// `GET /api/file?path=<entry>`
///
/// The Entry's plaintext, from the folder this device maps it into. Present here
/// already, and the file at its translated local path is it (spec: EP-9, EP-10);
/// not present, and the fetch that places it runs first (spec: EP-11). Either
/// way what the browser gets is a file on this device rather than anything
/// passed through from Storage: no ciphertext, no key, and no token crosses this
/// route.
///
/// Served whole. A range request is not honoured, because nothing asks for one —
/// a browser's `<img>` sends none — and honouring one would mean a second way of
/// reading an Entry with a second set of answers about a fetch that has to
/// happen first.
pub async fn file(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<Response, ApiError> {
    let path = query.entry()?;

    if state.library.state_of(&path).await? == EntryState::Present {
        let local = state.library.local_path_of(&path).await?;
        match fs::read(&local).await {
            Ok(bytes) => return Ok(served(&path, bytes, "present")),
            // The row says this device placed the file and the file is not there
            // now. That is a finding rather than a failure, and the fetch is
            // what states it (spec: EP-10, EP-11) — so the answer comes from
            // running one, which declines the path and says what it found.
            Err(refused) if refused.kind() == io::ErrorKind::NotFound => {}
            Err(refused) => return Err(ApiError::unreadable(refused)),
        }
    }

    match state.fetches.fetch(&state.library, path.clone()).await? {
        EntryFetch::Placed | EntryFetch::AlreadyPresent => {}
        EntryFetch::Surfaced(surfaced) => return Err(ApiError::declined(&surfaced)),
    }
    let local = state.library.local_path_of(&path).await?;
    let bytes = fs::read(&local).await.map_err(ApiError::unreadable)?;
    Ok(served(&path, bytes, "fetched"))
}

/// The plaintext, as what the classifier says it is.
fn served(path: &EntryPath, bytes: Vec<u8>, from: &'static str) -> Response {
    // The Entry Path is not in the event and never will be (spec: EP-1). What is
    // worth recording is that a request was answered, from where, and how much
    // it came to.
    info!(
        operation = "serve_file",
        from,
        bytes = bytes.len(),
        "served an Entry's plaintext",
    );
    (
        [
            (header::CONTENT_TYPE, classify(path).content_type),
            // The user's own plaintext. A shared cache must not keep it and a
            // browser must not write it to disk, which is what `no-store` says;
            // the spike's `public, max-age=86400` said the opposite of both.
            (header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes,
    )
        .into_response()
}
