//! What the binary says when asked which one it is.
//!
//! Spawned rather than driven as a router: `--version` is answered by the
//! argument parser before anything is served, so the built binary is the only
//! thing that can be asked.

use std::process::{Command, Stdio};

#[test]
fn the_binary_answers_version_with_the_workspace_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_coffret-server"))
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .expect("the built binary must be runnable");

    assert!(
        output.status.success(),
        "--version is an answer, not a refusal"
    );
    let said = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        said.trim(),
        format!("coffret-server {}", env!("CARGO_PKG_VERSION")),
        "the answer names the binary and the workspace's version"
    );
}
