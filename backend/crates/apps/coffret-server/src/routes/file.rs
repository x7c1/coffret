use std::io;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue};
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
            // below is what states it — as `LocallyChanged`, a pending local
            // change the sync flow owns rather than a file to put back
            // (spec: EP-10, EP-11).
            Ok(None) => {}
            // The row survived the Entry: another device removed the Container
            // the Entry lived in, and this device's file stays in the folder to
            // be reported rather than silently left behind (spec: EP-10). There
            // is no current Entry to translate, so nothing here can answer — but
            // the file is standing in the mapped folder, which is what the
            // listing is already showing it as, and what the next branch reads.
            Err(ref refusal) if row_outlived_its_entry(refusal) => {}
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
    // Both answers that reach here say the file was on this device a moment ago:
    // one was just renamed onto its name, and the other was opened to be
    // answered at all. So nothing there is left for this to be a verdict about,
    // and what it covers is the window between that moment and this call —
    // somebody removing the file out of the mapped folder in between. A server
    // refusal is the honest reading of that: the request was answerable when it
    // was decided, and the state it was decided on is gone.
    let file = library.open_local_file(&path).await?.ok_or_else(|| {
        ApiError::unreadable(io::Error::new(
            io::ErrorKind::NotFound,
            "the fetched file is no longer present",
        ))
    })?;
    Ok(served(&path, file, "fetched"))
}

/// Whether a refusal to open the placed file is the row having outlived the
/// Entry it was written for.
///
/// Named, because the branch above turns on it and nothing in the type system
/// keeps the two in step: the device layer chooses which of its refusals carries
/// the fetch's vocabulary for this reading, and a pattern here that stopped
/// matching would go on compiling — with the case falling through to the arm
/// below it and a row this device is meant to report becoming a failure instead.
/// So the pattern is written once and the case below holds it against the value
/// the call really makes.
fn row_outlived_its_entry(refusal: &Error) -> bool {
    matches!(
        refusal,
        Error::LocalFileNotOpened {
            cause: FetchError::EntryNotCurrent { .. },
        },
    )
}

/// The plaintext, as what the classifier says it is.
///
/// The answer is settled here and the reading happens after it, which is the one
/// thing streaming cost this route: a file that opens and then cannot be read
/// through — truncated under the reader, a disk that went wrong midway — is met
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
    // The Entry Path is not in the event and never will be (spec: EL-1). What is
    // worth recording is that a request was answered, from where, and how much
    // it came to.
    info!(
        operation = "serve_file",
        from, bytes, "served an Entry's plaintext",
    );
    let media = classify(path);
    let mut answer = Response::builder().header(header::CONTENT_TYPE, media.content_type);
    // Named only where the explorer does not draw the file. Bytes served as
    // `application/octet-stream` are ones a browser saves rather than shows, and
    // with nothing to go on it saves them as `file` with no extension — so the
    // answer says what they are called. A format the explorer draws gets no
    // header at all: the explorer reads those bytes itself to draw them, which
    // is what an answer with no disposition already means, and naming a file
    // nobody is saving would only put the person's own file name on every
    // picture's answer for nothing to read it.
    if !media.openable {
        answer = answer.header(header::CONTENT_DISPOSITION, attachment_named(path));
    }
    answer
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
            .inspect_err(record_stream_failure),
        ))
        .expect("a response built from constant or already-validated headers is well formed")
}

/// The `Content-Disposition` that saves a download under the Entry's own name.
///
/// The name is the last component of the Entry Path, which is the person's own
/// name for their file and nothing this server chose — so it arrives as
/// anything a name may be (spec: EP-1, EP-2), a quote or a line break included,
/// and the value is built so that none of that can end the header early or
/// start another. Built, not formatted: two spellings of one name, each
/// admitting only what its own grammar admits.
///
/// `filename*=UTF-8''…` carries the real name, percent-encoded per RFC 8187:
/// every byte outside the attribute characters that grammar allows travels as
/// `%XX`, so a quote, a semicolon or a line break is three inert characters.
/// Every current browser reads this one and prefers it.
///
/// `filename="…"` is the fallback for whatever reads only the older form, and a
/// quoted-string admits printable ASCII less `"` and `\`, which would end or
/// escape it. So anything else — a control character, a non-ASCII letter, the
/// two it cannot hold — becomes `_`, one per character: the name is legible and
/// the right length rather than exact, and the exact one is beside it.
fn attachment_named(path: &EntryPath) -> HeaderValue {
    let name = path.as_str().rsplit('/').next().unwrap_or(path.as_str());
    let fallback: String = name
        .chars()
        .map(|c| match c {
            ' '..='~' if c != '"' && c != '\\' => c,
            _ => '_',
        })
        .collect();
    let mut encoded = String::with_capacity(name.len());
    for byte in name.bytes() {
        match byte {
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'!'
            | b'#'
            | b'$'
            | b'&'
            | b'+'
            | b'-'
            | b'.'
            | b'^'
            | b'_'
            | b'`'
            | b'|'
            | b'~' => encoded.push(char::from(byte)),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"
    ))
    .expect("a value built from printable ASCII alone is a header value")
}

/// Records a body that stopped midway out, by what refused rather than by
/// which file it was about.
///
/// The Entry Path stays out of this event as it stays out of the one above
/// (spec: EL-1). Nor does the refusal's own message go in: a read off this
/// device's disk is reported as a local I/O refusal, and the `io::Error` under
/// one is free to be a custom error whose message repeats the path it was
/// refused on. What goes in is the refusal's log-safe rendering — the operation
/// and the error kind (spec: EL-3) — which is enough for the one thing worth
/// having here: that a request this server has already called answered did not
/// finish going out.
fn record_stream_failure(cause: &Error) {
    warn!(
        operation = "serve_file",
        error = %cause.redacted(),
        "an Entry's plaintext stopped part way out",
    );
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;

    use coffret_device::OpenLibrary;
    use coffret_local_fs::UnixFs;
    use coffret_logging::testing::CapturedLogs;
    use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
    use coffret_usecase::{
        InMemoryIndex, InMemoryStore, LibraryKeys, LocalIoError, LocalOperation,
    };
    use tracing::Level;

    use super::*;
    use crate::entry_paths::entry_path;

    /// A Library open over a catalog that holds nothing.
    ///
    /// Which is all the case below needs: an Entry Path the catalog has no
    /// current Entry at is exactly the state a row that outlived its Entry
    /// leaves behind, and nothing here reaches Storage or the disk to find that
    /// out.
    fn library_over_an_empty_catalog() -> OpenLibrary {
        OpenLibrary {
            store: Arc::new(InMemoryStore::new(64)),
            index: Arc::new(InMemoryIndex::new()),
            local_fs: Arc::new(UnixFs::new()),
            keys: LibraryKeys::derive(
                &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
                MasterKeyEpoch::FIRST,
            ),
            spool: std::env::temp_dir(),
            library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
            epoch: MasterKeyEpoch::FIRST,
            provider: "s3",
        }
    }

    // EP-10: another device removed the Container the Entry lived in, and the
    // row this device wrote when it placed the file outlives it. The route goes
    // on to serve the file as one of this device's own rather than failing, and
    // the whole of that reading is the branch this holds: the pattern is written
    // against the refusal the device layer really makes, so a variant renamed on
    // that side stops here rather than leaving an arm that quietly never matches
    // and a standing file answered as a failure.
    #[tokio::test]
    async fn a_row_that_outlived_its_entry_is_read_as_such_and_not_as_a_failure() {
        let answer = library_over_an_empty_catalog()
            .open_local_file(&entry_path("albums/spring.jpg"))
            .await;
        // Unwrapped by hand: an open file is not something a failure message can
        // be made of, so there is nothing for `expect_err` to print.
        let Err(refusal) = answer else {
            panic!("a catalog holding no current Entry cannot answer with a file");
        };

        assert!(
            row_outlived_its_entry(&refusal),
            "expected the branch that goes on to read the file to take this, got {refusal:?}",
        );
    }

    // A name is anything EP-2 does not exclude, and EP-2 excludes neither a line
    // break nor a backslash. Neither may end the header or escape the quoted
    // fallback: both become `_` there, and travel percent-encoded in the name
    // a browser actually uses.
    #[test]
    fn a_name_that_could_split_the_header_stays_inside_it() {
        let value = attachment_named(&entry_path("books/one\r\nSet-Cookie: x\\y.txt"));
        assert_eq!(
            value.to_str().expect("the value is ASCII"),
            "attachment; filename=\"one__Set-Cookie: x_y.txt\"; \
             filename*=UTF-8''one%0D%0ASet-Cookie%3A%20x%5Cy.txt",
        );
    }

    // EL-1, EL-3: a read that fails once the status and the length have gone
    // out cannot become a refusal, so this event is the only account of it. The
    // `io::Error` it carries is not always the operating system's own — a
    // custom one repeats the local path in its message — and the event keeps
    // the operation and the error kind and neither copy of that path.
    #[test]
    fn a_stream_that_stops_part_way_records_no_local_path() {
        const PRIVATE_PATH: &str = "/Users/alice/Pictures/Family Tax Records/receipt.pdf";
        let logs = CapturedLogs::capture();

        record_stream_failure(&Error::Local(LocalIoError::new(
            LocalOperation::Reading,
            PRIVATE_PATH,
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("the read of {PRIVATE_PATH} was denied"),
            ),
        )));

        let event = logs.only(Level::WARN);
        assert_eq!(event.field("operation"), "serve_file");
        assert_eq!(
            event.field("error"),
            "Device::Local: Local::Io(operation=read, kind=PermissionDenied)"
        );
        logs.assert_free_of(&[PRIVATE_PATH, "Family Tax Records"]);
    }
}
