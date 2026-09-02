use std::io;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use coffret_device::{EntryFetch, EntryPath, EntryState, Error, FetchError};
use tokio::fs;
use tracing::info;

use crate::api_error::ApiError;
use crate::classify::classify;
use crate::entry_query::PathQuery;
use crate::fill::fill_folder;
use crate::folder::Folder;
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
/// Three places the bytes may come from, in the order they are asked for: the
/// file this device placed for a current Entry, a file somebody added that no
/// sync has carried in yet, and a fetch. The first two are files already on this
/// device; the third is the only one that reaches Storage.
///
/// Served whole. A range request is not honoured, because nothing asks for one —
/// a browser's `<img>` sends none — and honouring one would mean a second way of
/// reading an Entry with a second set of answers about a fetch that has to
/// happen first.
pub async fn file(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<Response, ApiError> {
    // The keys, once, for the whole of this: the three places the bytes may come
    // from are one question about one Entry, and a lock that landed between two
    // of them would be a request answered out of half an answer (spec: DK-2).
    let library = state.unlocked()?;
    let path = query.entry()?;

    if library.state_of(&path).await? == EntryState::Present {
        match library.local_path_of(&path).await {
            Ok(local) => match fs::read(&local).await {
                Ok(bytes) => return Ok(served(&path, bytes, "present")),
                // The row says this device placed the file and the file is not
                // there now. That is a finding rather than a failure, and the
                // fetch is what states it (spec: EP-10, EP-11) — so the answer
                // comes from running one, which declines the path and says what
                // it found.
                Err(refused) if refused.kind() == io::ErrorKind::NotFound => {}
                Err(refused) => return Err(ApiError::unreadable(refused)),
            },
            // The row survived the Entry: another device removed the Container
            // the Entry lived in, and this device's file stays in the folder to
            // be reported rather than silently left behind (spec: EP-10). There
            // is no current Entry to translate, so nothing here can answer — but
            // the file is standing in the mapped folder, which is what the
            // listing is already showing it as, and what the next branch reads.
            Err(Error::Fetch {
                cause: FetchError::EntryNotCurrent { .. },
            }) => {}
            Err(cause) => return Err(cause.into()),
        }
    }

    // A file somebody added that no sync has carried in yet: it is in the mapped
    // folder, it is theirs, and the listing is already showing it. Reading it is
    // reading their own file — there is no Entry to fetch and nothing to be
    // declined about, and a reader that would not open it until a sync had run
    // would be refusing to show somebody what they had just put there.
    if let Some(local) = library.added_at(&path).await? {
        match fs::read(&local).await {
            Ok(bytes) => return Ok(served(&path, bytes, "added")),
            // It was there a moment ago and is not now. The Library holds no
            // Entry at the path either, so there is nothing left to answer with.
            Err(gone) if gone.kind() == io::ErrorKind::NotFound => {}
            Err(refused) => return Err(ApiError::unreadable(refused)),
        }
    }

    match state.fetches.fetch(&library, path.clone()).await? {
        // This device did not have the file and does now, which says something
        // about the folder around it: whoever opened this one is going to open
        // its neighbours. So the rest of the folder is brought over in the
        // background from here — a fetch having been necessary is the whole of
        // the signal, because fetching is implicit and there is no button that
        // asks for it.
        EntryFetch::Placed => fill_folder(Arc::clone(&state), Folder::holding(&path)),
        EntryFetch::AlreadyPresent => {}
        EntryFetch::Surfaced(surfaced) => return Err(ApiError::declined(&surfaced)),
    }
    let local = library.local_path_of(&path).await?;
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
