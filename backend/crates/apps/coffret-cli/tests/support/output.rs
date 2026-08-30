//! Reading back what a run said, and what it exited with.

use std::process::Output;

use super::RECOVERY_CODE_PREFIX;

/// What a run wrote to standard output.
pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// What a run wrote to standard error.
pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The status a run exited with, for a case that says which one it expects.
pub fn code(output: &Output) -> i32 {
    output
        .status
        .code()
        .expect("a run must exit rather than be signalled")
}

/// Asserts that a run succeeded with nothing left to act on.
pub fn succeeded(output: &Output, what: &str) {
    assert_eq!(
        code(output),
        0,
        "{what} must succeed with no findings; stderr was:\n{}\nstdout was:\n{}",
        stderr(output),
        stdout(output)
    );
}

/// The Recovery Code a run printed, as one line.
pub fn printed_code(output: &Output) -> String {
    stdout(output)
        .lines()
        .find(|line| line.starts_with(RECOVERY_CODE_PREFIX))
        .unwrap_or_else(|| {
            panic!(
                "no Recovery Code on standard output; stderr was:\n{}",
                stderr(output)
            )
        })
        .to_owned()
}

/// Where `init` said it put the Library, as the `s3://bucket/prefix` it printed.
///
/// Read back off the run rather than out of the settings file, because that is
/// what a person joining from a second device has: the line `init` printed
/// (spec: FM-18).
pub fn printed_prefix(output: &Output) -> String {
    let line = stderr(output)
        .lines()
        .find(|line| line.contains("s3://"))
        .unwrap_or_else(|| panic!("init must say where the Library went:\n{}", stderr(output)))
        .to_owned();

    let (_, location) = line.split_once("s3://").expect("the line holds the scheme");
    let (_, prefix) = location
        .split_once('/')
        .expect("a Library's location is a bucket and a prefix");
    prefix.to_owned()
}
