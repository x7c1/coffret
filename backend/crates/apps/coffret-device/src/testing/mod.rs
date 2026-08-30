//! Helpers shared by this crate's own tests.
//!
//! Creating an S3 Library reaches no network at all — on S3 a prefix exists by
//! being written under, so nothing is created until the first commit — which is
//! what lets the whole of the creation flow, the layout it produces, and the
//! refusals it owes be tested in this crate rather than only behind a
//! container. What needs a real bucket is opening one, and that is in `tests/`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::create_library::{create_library, CreateLibraryRequest, CreatedLibrary, NewProvider};
use crate::library_dir::STATE_DIRECTORY;

/// The Passphrase every case here uses.
pub(crate) const PASSPHRASE: &[u8] = b"correct horse battery staple";

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

/// Creates an S3 Library called `name`, which reaches no network.
pub(crate) async fn create_s3(name: &str) -> CreatedLibrary {
    state_dir();
    create_library(request(name), |_| {
        panic!("an S3 Library asks nobody for consent")
    })
    .await
    .expect("an S3 Library needs nothing but this device")
}

/// What every case here asks for.
pub(crate) fn request(name: &str) -> CreateLibraryRequest {
    CreateLibraryRequest {
        name: name.to_owned(),
        passphrase: PASSPHRASE.to_vec(),
        provider: NewProvider::S3 {
            bucket: "photos".to_owned(),
            base_prefix: "archive/".to_owned(),
            endpoint: None,
            region: None,
            path_style: false,
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
