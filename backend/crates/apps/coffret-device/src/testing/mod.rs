//! Helpers shared by this crate's own tests.
//!
//! Creating an S3 Library writes nothing to Storage — on S3 a prefix exists by
//! being written under, so nothing is there until the first commit — and asks it
//! exactly one question: whether the bucket is there at all. That is what lets
//! the whole of the creation flow, the layout it produces, and the refusals it
//! owes be tested in this crate rather than only behind a container, with
//! [`stub_bucket`] standing in for the bucket. What needs a real one is
//! opening a Library and running a flow over it, and those are in `tests/`.

mod drive_stub;
pub(crate) use drive_stub::{consent, DriveStub, CREATED_FOLDER_ID};

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use coffret_local_fs::UnixFs;
use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker;

use crate::create_library::{create_library, CreateLibraryRequest, CreatedLibrary, NewProvider};
use crate::error::Result;
use crate::library_dir::STATE_DIRECTORY;
use crate::stub_bucket::stub_endpoint;

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

/// A catalog opener that is refused, the way a disk that will not hold the
/// Index file refuses it.
///
/// A backend fault rather than anything about the file's contents: the
/// catalog is brand new, so the only thing that can stop it is the store under
/// it. Handed to [`Reach::opening_index_with`](crate::reach::Reach) by the
/// cases over a Library whose catalog never came to exist.
pub(crate) fn unopenable_catalog(_: &Path) -> coffret_usecase::IndexResult<()> {
    Err(coffret_usecase::IndexError::Backend {
        operation: "opening the Index file",
        cause: Box::new(std::io::Error::other("the disk refused the catalog's file")),
    })
}

/// The OAuth client every Drive case authorizes as.
pub(crate) const CLIENT_ID: &str = "stub-client.apps.googleusercontent.com";

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

/// The stub bucket's endpoint, with something for the SDK to sign for it with.
///
/// Whatever the SDK resolves has to be *something* for a request to be signed
/// at all, and on a machine with none configured the resolution itself is what
/// would fail the case. What is signed with is never checked by
/// [`stub_endpoint`], which is why the credentials set here are made up, and set
/// only where nothing else already is.
pub(crate) fn stub_bucket() -> &'static str {
    static SIGNED: OnceLock<()> = OnceLock::new();
    SIGNED.get_or_init(|| {
        for (name, value) in [
            ("AWS_ACCESS_KEY_ID", "coffret-device-tests"),
            ("AWS_SECRET_ACCESS_KEY", "coffret-device-tests-secret"),
            ("AWS_REGION", REGION),
        ] {
            if std::env::var_os(name).is_none() {
                std::env::set_var(name, value);
            }
        }
    });
    stub_endpoint()
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
            endpoint: Some(stub_bucket().to_owned()),
            region: Some(REGION.to_owned()),
            path_style: true,
        },
        referencing_passphrase: crate::ReferencingPassphrase::unasked(),
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
