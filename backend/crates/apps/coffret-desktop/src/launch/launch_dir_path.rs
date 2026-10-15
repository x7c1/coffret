use std::env;
use std::path::{Component, Path, PathBuf};

use anyhow::Context;

/// The directory under the state directory that launch files are kept in
/// where there is no runtime directory to keep them in.
const UNDER_STATE: &str = "explorer-launch";

/// Where the app with the identifier `identifier` writes its launch files
/// (spec: LA-21).
///
/// On Linux, `$XDG_RUNTIME_DIR/<identifier>`: a per-user directory of mode
/// `0700` on a tmpfs, which the session removes at logout, so a file a crash
/// left behind does not outlive the session. Where that is unset, and on
/// macOS, `explorer-launch/<identifier>` under the state directory the
/// binaries keep Libraries under. Either way it is the identifier's own, so the
/// installed app and the development app each sweep only theirs.
pub fn launch_dir_path(identifier: &str) -> anyhow::Result<PathBuf> {
    let runtime = if cfg!(target_os = "linux") {
        env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
    } else {
        None
    };
    choose(runtime, identifier, || {
        coffret_device::state_directory().context("the state directory could not be found")
    })
}

/// The directory for `identifier`, under `runtime` where that is a directory
/// the XDG specification allows — set, and absolute — and otherwise under what
/// `state` answers.
pub(super) fn choose(
    runtime: Option<PathBuf>,
    identifier: &str,
    state: impl FnOnce() -> anyhow::Result<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let mut components = Path::new(identifier).components();
    anyhow::ensure!(
        matches!(
            (components.next(), components.next()),
            (Some(Component::Normal(_)), None)
        ) && !identifier.starts_with('.'),
        "the app's identifier {identifier:?} is not one directory name"
    );
    match runtime.filter(|runtime| runtime.is_absolute()) {
        Some(runtime) => Ok(runtime.join(identifier)),
        None => Ok(state()?.join(UNDER_STATE).join(identifier)),
    }
}
