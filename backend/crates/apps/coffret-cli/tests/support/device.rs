//! One device, and what a case puts in the folders it maps.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use super::minio::signing_credentials;

/// One device: a state directory of its own, and the folders it maps.
pub struct Device {
    state: TempDir,
    folders: TempDir,
}

impl Device {
    pub fn new() -> Self {
        signing_credentials();
        Self {
            state: TempDir::new().expect("a temporary directory must be available"),
            folders: TempDir::new().expect("a temporary directory must be available"),
        }
    }

    /// Runs `coffret` with nothing on standard input.
    pub fn run(&self, arguments: &[&str]) -> Output {
        self.run_with(arguments, None)
    }

    /// Runs `coffret`, offering `input` on standard input.
    pub fn run_with(&self, arguments: &[&str], input: Option<&str>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_coffret"))
            .args(arguments)
            .env("COFFRET_STATE_DIR", self.state.path())
            // The logs go to the same throwaway directory, so a run leaves
            // nothing in the state directory of whoever started it.
            .env("COFFRET_LOG_DIR", self.state.path().join("logs"))
            // The Drive variables are taken out rather than left to whatever
            // started the run. A machine set up for the Drive targets has a
            // real client and a real secret in its environment, and a child
            // that inherited them would be running these cases against a
            // different configuration from the one CI runs them in — a case
            // about an absent secret passing here and failing there, or the
            // other way about. Nothing under test reads either one except
            // through what a case names, so removing them is the environment
            // the cases describe.
            .env_remove("COFFRET_DRIVE_CLIENT_ID")
            .env_remove("COFFRET_DRIVE_CLIENT_SECRET")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the built binary must be runnable");

        let mut stdin = child.stdin.take().expect("standard input was piped");
        if let Some(input) = input {
            // A run that refuses before asking never reads this, and the pipe
            // closing under it is what that looks like from here.
            let _ = writeln!(stdin, "{input}");
        }
        drop(stdin);

        child.wait_with_output().expect("the run must finish")
    }

    /// Where Libraries are on this device.
    pub fn libraries(&self) -> PathBuf {
        self.state.path().join("libraries")
    }

    /// The folder at `name`, created.
    pub fn folder(&self, name: &str) -> PathBuf {
        let path = self.folders.path().join(name);
        std::fs::create_dir_all(&path).expect("the folder must be creatable");
        path
    }
}

/// Writes `contents` to `path`, creating what is above it.
pub fn write_file(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("the folder must be creatable");
    }
    std::fs::write(path, contents).expect("the file must be writable");
}
