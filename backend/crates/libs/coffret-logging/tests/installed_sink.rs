//! What installing the sink actually leaves on disk.
//!
//! A test target of its own, because installing is a once-per-process act: the
//! subscriber it sets is the global one, and a second install in the same
//! process would be refused. So there is one case here, and it is the whole
//! path — settings, directory, file, permissions, and an event arriving in it.

use std::fs;

use coffret_logging::{install, LogSettings};
use tempfile::TempDir;
use tracing::Level;

#[test]
fn an_installed_sink_writes_events_to_a_file_only_its_owner_can_read() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    let settings = LogSettings::new(directory.path()).with_level(Level::DEBUG);

    let path = install(&settings).expect("a temporary directory must be writable");

    // Targets are named the way the crates that really emit these are, because
    // the target is half of what decides whether an event reaches the file.
    tracing::info!(target: "s3_store::s3", object = "jrn-7.cfrt", "stored an object");
    tracing::debug!(target: "s3_store::s3", status = 200, "a call was answered");
    tracing::trace!(target: "s3_store::s3", "more than was asked for");
    // A dependency narrating its own internals. The ceiling is shared, so
    // letting this in by default would evict the evidence above it.
    tracing::debug!(
        target: "aws_smithy_runtime::client::orchestrator",
        "timeout settings for this operation",
    );

    let written = fs::read_to_string(&path).expect("the log file must be readable");
    assert!(written.contains("stored an object"), "{written}");
    assert!(written.contains("jrn-7.cfrt"), "{written}");
    assert!(written.contains("a call was answered"), "{written}");
    assert!(
        !written.contains("more than was asked for"),
        "the level was not honoured: {written}",
    );
    assert!(
        !written.contains("timeout settings"),
        "a dependency was allowed to spend the ceiling: {written}",
    );

    // The path the settings named, not one invented somewhere else: whoever
    // goes looking for the evidence has to find it where it was documented.
    assert!(path.starts_with(directory.path()), "{path:?}");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&path)
            .expect("the log file must exist")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600, "{path:?} is not owner-only");
    }
}
