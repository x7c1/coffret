use std::path::Path;

use anyhow::Context;
use coffret_explorer_host::launch_file::LaunchDir;

use super::io_error_kind;

/// The owner-only directory at `path`, made where it is missing and refused
/// where it is not this account's alone, emptied of whatever launch files a
/// run before this one left in it (spec: LA-21).
///
/// Done as the app starts, before a Passphrase is typed, so that a directory a
/// person has to fix is said before anything else. A stale file that will not
/// go is said in the log and does not stop the app: the ticket in it is of a
/// run that is over, and admits nobody (spec: LA-17, LA-18).
pub fn prepare_launch_dir(path: &Path) -> anyhow::Result<LaunchDir> {
    let dir = LaunchDir::prepare(path)
        .with_context(|| format!("{} cannot hold launch files", path.display()))?;
    match dir.sweep() {
        Ok(0) => {}
        Ok(removed) => tracing::info!(
            kind = "launch_file_removed",
            reason = "stale",
            removed,
            "launch files an earlier run left were removed",
        ),
        Err(error) => tracing::warn!(
            kind = "launch_file_not_removed",
            reason = "stale",
            io_error_kind = io_error_kind(&error),
            "a launch file an earlier run left could not be removed",
        ),
    }
    Ok(dir)
}
