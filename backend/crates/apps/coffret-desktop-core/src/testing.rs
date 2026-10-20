//! What this crate's cases share: a state directory of their own, and
//! Libraries created in it against the device crate's loopback bucket.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use coffret_device::{
    create_library, stub_endpoint, CreateLibraryRequest, NewProvider, Passphrase,
    ReferencingPassphrase, STATE_DIRECTORY,
};

use crate::Desktop;

/// The Passphrase every case's Library is created with.
pub(crate) const PASSPHRASE: &str = "correct horse battery staple";

/// The identifier every case's desktop is started for.
pub(crate) const IDENTIFIER: &str = "io.github.x7c1.coffret.test";

/// The region the loopback bucket is signed for.
const REGION: &str = "us-east-1";

/// The state directory every case in this binary runs under: a temporary one,
/// and never the device's own.
///
/// One for the whole binary, because the directory is named by an environment
/// variable and a variable is one value for a process. Cases are told apart by
/// the names of their Libraries instead. Every case that reads the environment
/// starts here, so that nothing reads it while it is being set.
pub(crate) fn state_dir() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        let path = directory.keep();
        std::env::set_var(STATE_DIRECTORY, &path);
        // Whatever the SDK resolves has to be something for a request to the
        // loopback bucket to be signed at all; the bucket checks none of it.
        for (name, value) in [
            ("AWS_ACCESS_KEY_ID", "coffret-desktop-core-tests"),
            ("AWS_SECRET_ACCESS_KEY", "coffret-desktop-core-tests-secret"),
            ("AWS_REGION", REGION),
        ] {
            if std::env::var_os(name).is_none() {
                std::env::set_var(name, value);
            }
        }
        path
    })
    .as_path()
}

/// A desktop of the case's own, every task of it on the thread that drives
/// its runtime, under the cases' state directory.
pub(crate) fn desktop() -> Desktop {
    state_dir();
    Desktop::on_current_thread(IDENTIFIER, 30).expect("a desktop starts")
}

/// Creates a Library called `name` in the cases' state directory, with
/// [`PASSPHRASE`].
pub(crate) async fn create(name: &str) {
    state_dir();
    let request = CreateLibraryRequest {
        name: name.to_owned(),
        provider: NewProvider::S3 {
            bucket: "desktop".to_owned(),
            base_prefix: format!("{name}/"),
            endpoint: Some(stub_endpoint().to_owned()),
            region: Some(REGION.to_owned()),
            path_style: true,
        },
        referencing_passphrase: ReferencingPassphrase::unasked(),
    };
    create_library(
        request,
        || Ok(Passphrase::from_bytes(PASSPHRASE.as_bytes().to_vec())),
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await
    .expect("an S3 Library needs nothing but this device and a bucket that answers");
}
