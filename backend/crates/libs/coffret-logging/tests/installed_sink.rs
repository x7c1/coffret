//! What installing the sink actually leaves on disk.
//!
//! A test target of its own, because installing is a once-per-process act: the
//! subscriber it sets is the global one, and a second install in the same
//! process would be refused. So there is one case here, and it is the whole
//! path — settings, directory, file, permissions, and an event arriving in it.

use std::fs;

use coffret_logging::{install, LogSettings};
use serde_json::Value;
use tempfile::TempDir;
use tracing::Level;

/// Every key a record is allowed to carry, in alphabetical order.
///
/// Asserted on rather than assumed: the formatter can be asked for a source
/// file, a line number, a span's fields, or a thread's name, and each of those
/// is something reaching the file that no rule about events covers. A record
/// that grows a key comes back through here first.
///
/// Alphabetical rather than as written, because which of the two a parser hands
/// back is a build's choice — `serde_json` keeps insertion order only where
/// something in the tree asked it to — and the question here is which keys are
/// there.
const RECORD_KEYS: [&str; 4] = ["fields", "level", "target", "timestamp"];

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
    // One JSON object per line, which is the whole point of the format: what
    // reaches the file is asked for by name below rather than searched for.
    let records: Vec<Value> = written
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}")))
        .collect();

    let [stored, answered] = records.as_slice() else {
        panic!("two events were emitted above the filters, and no others: {written}");
    };

    assert_eq!(stored["level"], "INFO");
    assert_eq!(stored["target"], "s3_store::s3");
    assert_eq!(stored["fields"]["message"], "stored an object");
    assert_eq!(stored["fields"]["object"], "jrn-7.cfrt");

    assert_eq!(answered["level"], "DEBUG");
    assert_eq!(answered["fields"]["message"], "a call was answered");
    // A number, and not the string "200": a field keeps the type it was
    // recorded with, which is what lets a reader compare and sum them.
    assert_eq!(answered["fields"]["status"], 200);

    for record in &records {
        let mut keys: Vec<&str> = record
            .as_object()
            .expect("a record is a JSON object")
            .keys()
            .map(String::as_str)
            .collect();

        keys.sort_unstable();
        assert_eq!(keys, RECORD_KEYS, "a record grew a key: {record}");
    }

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
