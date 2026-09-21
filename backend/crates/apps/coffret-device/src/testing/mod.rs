//! Helpers shared by this crate's own tests.
//!
//! Creating an S3 Library writes nothing to Storage — on S3 a prefix exists by
//! being written under, so nothing is there until the first commit — and asks it
//! exactly one question: whether the bucket is there at all. That is what lets
//! the whole of the creation flow, the layout it produces, and the refusals it
//! owes be tested in this crate rather than only behind a container, with
//! [`stub_endpoint`] standing in for the bucket. What needs a real one is
//! opening a Library and running a flow over it, and those are in `tests/`.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use coffret_local_fs::UnixFs;
use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker;

use crate::create_library::{create_library, CreateLibraryRequest, CreatedLibrary, NewProvider};
use crate::error::Result;
use crate::library_dir::STATE_DIRECTORY;

/// `error` and every link beneath it, joined the way a caller printing
/// `{error:#}` reads them.
///
/// A wrapper names only its own layer in `Display` and leaves what the layer
/// below answered to `source`, so a bare `{error}` in a panic drops everything
/// under the top line, which is usually the part that says what actually went
/// wrong.
///
/// Named apart from the `chain` this crate's error cases keep: that one hands
/// the links back one at a time, to be asserted over, and this one renders them
/// for somebody reading a panic.
pub(crate) fn every_link(error: &dyn std::error::Error) -> String {
    let mut rendered = error.to_string();
    let mut cause = error.source();
    while let Some(link) = cause {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        cause = link.source();
    }
    rendered
}

/// The Entry Path `text` spells, or a panic naming the literal that does not
/// spell one.
///
/// Every Entry Path is built by parsing text (spec: EP-1, EP-2), and a fixture
/// is no exception: what a test writes down is text like any other, and the
/// type has no constructor that takes it on trust. The unwrap lives here rather
/// than at each of the fixtures so that a literal somebody mistypes is reported
/// once, as the mistake in the fixture that it is.
pub(crate) fn entry_path(text: impl Into<String>) -> EntryPath {
    EntryPath::parse(text)
        .unwrap_or_else(|error| panic!("a fixture holds a literal Entry Path: {error}"))
}

/// The disk an [`OpenLibrary`](crate::OpenLibrary) a case builds by hand spools
/// onto.
///
/// The real one, as `open_library` builds it: it holds nothing, so a case that
/// never spools pays nothing for having it, and a case that does spools the way
/// this device does.
pub(crate) fn local_fs() -> Arc<UnixFs> {
    Arc::new(UnixFs::new())
}

/// Gives a mapped root the marker a placement compares against, and hands back
/// the identity its mapping has to record (spec: EP-13).
///
/// A case that builds its own [`OpenLibrary`](crate::OpenLibrary) records its
/// mappings straight into a catalog rather than through
/// [`set_mapping`](crate::set_mapping), which is the one call in coffret that
/// writes a marker — so it arranges the root here instead. The bytes are the
/// format's own ([`root_marker::spell`]), so a root arranged this way is the root
/// a real registration leaves behind.
pub(crate) fn register_root(root: &Path) -> RootMarkerId {
    let id = RootMarkerId::from_bytes([0x2a; RootMarkerId::BYTE_LEN]);
    let area = root.join(root_marker::MANAGEMENT_AREA);
    std::fs::create_dir_all(&area).expect("making a case's management area must succeed");
    std::fs::write(area.join(root_marker::MARKER_FILE), root_marker::spell(&id))
        .expect("writing a case's marker must succeed");
    id
}

/// The Passphrase every case here uses.
pub(crate) const PASSPHRASE: &[u8] = b"correct horse battery staple";

/// The region every case signs for.
///
/// Fixed rather than resolved: a signature needs one, and which region a case's
/// bucket would be in is not what any case here is about.
pub(crate) const REGION: &str = "us-east-1";

/// The Passphrase, as the flows ask for it.
pub(crate) fn passphrase() -> Result<Passphrase> {
    Ok(Passphrase::from_bytes(PASSPHRASE.to_vec()))
}

/// The state directory every case in this binary runs under.
///
/// One for the whole binary rather than one per case, because the directory is
/// named by an environment variable and a variable is one value for a process.
/// Cases are told apart by device-local Library name instead, which is what
/// they would be told apart by on a real device anyway.
pub(crate) fn state_dir() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let path = directory.keep();
        std::env::set_var(STATE_DIRECTORY, &path);
        path
    })
    .as_path()
}

/// An endpoint that answers the two questions putting an S3 Library on this
/// device asks.
///
/// Creating a Library asks its bucket whether it is there, which is what turns a
/// mistyped bucket into a refusal at `init` rather than a surprise at the first
/// sync. Joining one asks that and then whether the prefix it was given holds
/// the first link of a Library's head chain, which is what turns a mistyped
/// Library ID into a word at `join` rather than into a `fetch` that reports
/// nothing forever. Both have to be answered for the cases here to be about
/// anything else, and a container is far more than answering them takes.
///
/// So this is a socket that says `200` to a request addressed at the bucket and
/// `404` to one addressed at a key under it — which is exactly what a bucket
/// that exists and has never been written into answers, and what every Library
/// these cases create is: creating one writes nothing to Storage.
///
/// It says nothing else about S3 and is not meant to. What a real
/// implementation answers is the conformance suites' business, and those run
/// against MinIO.
pub(crate) fn stub_endpoint() -> &'static str {
    static ENDPOINT: OnceLock<String> = OnceLock::new();
    ENDPOINT
        .get_or_init(|| {
            // Whatever the SDK resolves has to be *something* for a request to be
            // signed at all, and on a machine with none configured the resolution
            // itself is what would fail the case. What is signed with is never
            // checked here.
            for (name, value) in [
                ("AWS_ACCESS_KEY_ID", "coffret-device-tests"),
                ("AWS_SECRET_ACCESS_KEY", "coffret-device-tests-secret"),
                ("AWS_REGION", REGION),
            ] {
                if std::env::var_os(name).is_none() {
                    std::env::set_var(name, value);
                }
            }

            let listener = TcpListener::bind("127.0.0.1:0")
                .expect("a loopback port must be available for the stub bucket");
            let endpoint = format!(
                "http://{}",
                listener
                    .local_addr()
                    .expect("a bound listener has an address")
            );

            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    std::thread::spawn(move || answer(stream));
                }
            });
            endpoint
        })
        .as_str()
}

/// Answers every request one connection carries, until it closes.
fn answer(stream: TcpStream) {
    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        // Nothing to report it to and nothing that depends on it: a case whose
        // bucket did not answer fails on its own account.
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    let mut about_a_key = false;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return,
            Ok(_) => {}
            Err(_) => return,
        }
        // The first line of a request carries what it is about, and it is the
        // only part of the head worth reading here.
        if let Some(target) = request_target(&line) {
            about_a_key = names_a_key(target);
            continue;
        }
        // The request's head ends at the blank line; nothing here reads a body,
        // because every call made against this is a `HEAD`.
        if line.trim().is_empty() {
            let answer: &[u8] = match about_a_key {
                true => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n",
                false => b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n",
            };
            if writer
                .write_all(answer)
                .and_then(|()| writer.flush())
                .is_err()
            {
                return;
            }
        }
    }
}

/// What a request line is addressed at, where the line is one.
fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    (version.starts_with("HTTP/") && method.chars().all(|c| c.is_ascii_uppercase()))
        .then_some(target)
}

/// Whether a request target names a key inside the bucket rather than the
/// bucket itself.
///
/// Every case here addresses the bucket as a path segment, so the bucket alone
/// is one segment and anything under it is more than one.
fn names_a_key(target: &str) -> bool {
    target
        .split('?')
        .next()
        .unwrap_or(target)
        .trim_matches('/')
        .contains('/')
}

/// Creates an S3 Library called `name` against the stub bucket.
pub(crate) async fn create_s3(name: &str) -> CreatedLibrary {
    state_dir();
    create_library(request(name), passphrase, |_| {
        panic!("an S3 Library asks nobody for consent")
    })
    .await
    .expect("an S3 Library needs nothing but this device and a bucket that answers")
}

/// What every case here asks for.
pub(crate) fn request(name: &str) -> CreateLibraryRequest {
    CreateLibraryRequest {
        name: name.to_owned(),
        provider: NewProvider::S3 {
            bucket: "photos".to_owned(),
            base_prefix: "archive/".to_owned(),
            endpoint: Some(stub_endpoint().to_owned()),
            region: Some(REGION.to_owned()),
            path_style: true,
        },
    }
}

/// The mode a file is kept at.
#[cfg(unix)]
pub(crate) fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .unwrap_or_else(|error| panic!("{} must be there: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}
