use std::io;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header;
use axum::response::Response;
use coffret_device::{EntryFetch, EntryPath, EntryState, Error, FetchError, LocalFile, Redacted};
use futures_util::{stream, TryStreamExt};
use tracing::{info, warn};

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
///
/// Whole is not the same as at once. The file is handed to the response as an
/// open reader and goes out as it is read, so what this route costs in memory is
/// one buffer whatever the Entry is — and the Library puts no ceiling on what an
/// Entry may be (spec: PK-3). Reading it into a `Vec` first would have made the
/// server's memory a function of somebody's largest scan, on the one route whose
/// product exists to store such things.
pub async fn file(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<Response, ApiError> {
    // The keys, once, for the whole of this: the three places the bytes may come
    // from are one question about one Entry, and a lock that landed between two
    // of them would be a request answered out of half an answer (spec: DK-2).
    //
    // They are given up where this returns, and the bytes go out after that. That
    // is not a hole: what is left by then is an open handle on a plaintext file
    // of this device's own, which no key of the Library opens and no lock closes.
    // Every decision that needed one — whether the Entry is current, where it
    // stands, whether to fetch it — was made while this was held.
    let library = state.unlocked()?;
    let path = query.entry()?;

    if library.state_of(&path).await? == EntryState::Present {
        match library.open_local_file(&path).await {
            Ok(Some(file)) => return Ok(served(&path, file, "present")),
            // The row says this device placed the file and the file is not
            // there now. That is a finding rather than a failure, and the fetch
            // below is what states it (spec: EP-10, EP-11).
            Ok(None) => {}
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
    if let Some(file) = library.added_at(&path).await? {
        return Ok(served(&path, file, "added"));
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
    let file = library.open_local_file(&path).await?.ok_or_else(|| {
        ApiError::unreadable(io::Error::new(
            io::ErrorKind::NotFound,
            "the fetched file is no longer present",
        ))
    })?;
    Ok(served(&path, file, "fetched"))
}

/// The plaintext, as what the classifier says it is.
///
/// The answer is settled here and the reading happens after it, which is the one
/// thing streaming cost this route: a file that opens and then cannot be read
/// through — truncated under the reader, a disk that went wrong part way — is met
/// once the status and the length have already gone out, so it cannot become a
/// refusal the way an unopenable file still does. What the caller gets is a
/// transfer that ends short of the length it was promised. The explorer asks for
/// these bytes rather than pointing an `<img>` at them, so what a reader meets
/// is a request that failed after it had been answered — its own sentence, and
/// its own offer to try again, which is what recovers this where the file can be
/// read after all. A caller that did point an `<img>` at the route gets a broken
/// image and no sentence at all. The one account of why is the line below: with
/// nothing there, a `serve_file` that says it served something would be
/// indistinguishable from one that did.
fn served(path: &EntryPath, file: LocalFile, from: &'static str) -> Response {
    let bytes = file.len();
    // The Entry Path is not in the event and never will be (spec: EP-1). What is
    // worth recording is that a request was answered, from where, and how much
    // it came to.
    info!(
        operation = "serve_file",
        from, bytes, "served an Entry's plaintext",
    );
    Response::builder()
        .header(header::CONTENT_TYPE, classify(path).content_type)
        // The user's own plaintext. A shared cache must not keep it and a
        // browser must not write it to disk, which is what `no-store` says;
        // the spike's `public, max-age=86400` said the opposite of both.
        .header(header::CACHE_CONTROL, "private, no-store")
        // Stated, because a streamed body cannot be measured by whoever encodes
        // it: without this the answer goes out chunked and the browser is told
        // nothing about how much is coming. It is known here, off the handle the
        // file was measured from, so there is nothing to be gained by leaving it
        // out — and a `HEAD` of this route is answered out of it.
        .header(header::CONTENT_LENGTH, bytes)
        .body(Body::from_stream(
            stream::try_unfold(file, |mut file| async move {
                let mut buffer = vec![0; 64 * 1024];
                let filled = file.read(&mut buffer).await?;
                if filled == 0 {
                    return Ok::<Option<(Vec<u8>, LocalFile)>, coffret_device::Error>(None);
                }
                buffer.truncate(filled);
                Ok(Some((buffer, file)))
            })
            .inspect_err(|cause| {
                // The Entry Path stays out of this event as it stays out of the
                // one above (spec: EP-1), and an `io::Error` off a read names no
                // file either. What is worth having is that a request this
                // server has already called answered did not finish going out.
                warn!(
                    operation = "serve_file",
                    error = %cause.redacted(),
                    "an Entry's plaintext stopped part way out",
                );
            }),
        ))
        .expect("a response built from constant headers is well formed")
}
