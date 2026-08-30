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
use std::sync::OnceLock;

use crate::create_library::{create_library, CreateLibraryRequest, CreatedLibrary, NewProvider};
use crate::error::Result;
use crate::library_dir::STATE_DIRECTORY;

/// The Passphrase every case here uses.
pub(crate) const PASSPHRASE: &[u8] = b"correct horse battery staple";

/// The region every case signs for.
///
/// Fixed rather than resolved: a signature needs one, and which region a case's
/// bucket would be in is not what any case here is about.
pub(crate) const REGION: &str = "us-east-1";

/// The Passphrase, as the flows ask for it.
pub(crate) fn passphrase() -> Result<Vec<u8>> {
    Ok(PASSPHRASE.to_vec())
}

/// The state directory every case in this binary runs under.
///
/// One for the whole binary rather than one per case, because the directory is
/// named by an environment variable and a variable is one value for a process.
/// Cases are told apart by Library name instead, which is what they would be on
/// a real device anyway.
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

/// An endpoint that answers the one question creating an S3 Library asks.
///
/// Creating a Library asks its bucket whether it is there, which is what turns a
/// mistyped bucket into a refusal at `init` rather than a surprise at the first
/// sync. That question has to be answered for the cases here to be about
/// anything else, and a container is far more than answering it takes: this is a
/// socket that says `200` to whatever arrives, which is the whole of what
/// `HeadBucket` on a bucket that exists comes back as.
///
/// It says nothing about S3 and is not meant to. What a real implementation
/// answers is the conformance suites' business, and those run against MinIO.
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
                    std::thread::spawn(move || answer_ok(stream));
                }
            });
            endpoint
        })
        .as_str()
}

/// Says `200` to every request one connection carries, until it closes.
fn answer_ok(stream: TcpStream) {
    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        // Nothing to report it to and nothing that depends on it: a case whose
        // bucket did not answer fails on its own account.
        Err(_) => return,
    };
    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return,
            Ok(_) => {}
            Err(_) => return,
        }
        // The request's head ends at the blank line; nothing here reads a body,
        // because the only call made against this is a `HEAD`.
        if line.trim().is_empty()
            && writer
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n")
                .and_then(|()| writer.flush())
                .is_err()
        {
            return;
        }
    }
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
