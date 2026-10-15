use std::path::Path;

use anyhow::Context;
use tauri::Url;

/// What the system is asked to open for the launch file at `file`: its
/// `file:` URL, and nothing else.
///
/// This is what reaches the browser's command line, which another account on
/// the device can read, so it is made from the file's path alone and never
/// from what the file holds (spec: LA-21).
pub fn browser_target(file: &Path) -> anyhow::Result<Url> {
    Url::from_file_path(file)
        .ok()
        .context("the launch file's path is not an absolute path")
}
