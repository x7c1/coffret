//! Driving the built binary the way a person does.
//!
//! The cases run `coffret` itself rather than calling into `coffret-device`,
//! because what is being checked is the shell: that the flags parse into the
//! call they claim to, that the Recovery Code reaches standard output where a
//! caller can pipe it, and that a refusal is an exit status and not a line of
//! prose on standard output that a script would mistake for an answer.
//!
//! An S3 Library is what they create, and creating one reaches no network at
//! all: on S3 a prefix exists by being written under, so nothing exists until
//! the first commit. That is what lets the whole of `init`, `map`, `mappings`
//! and `recovery-code` be exercised in an ordinary test run.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

/// The Passphrase every case here uses.
const PASSPHRASE: &str = "correct horse battery staple";

/// What a Recovery Code starts with, printed or not (spec: KD-11).
const RECOVERY_CODE_PREFIX: &str = "coffret1";

/// One device: a state directory of its own, and the folders it maps.
struct Device {
    state: TempDir,
    folders: TempDir,
}

impl Device {
    fn new() -> Self {
        Self {
            state: TempDir::new().expect("a temporary directory must be available"),
            folders: TempDir::new().expect("a temporary directory must be available"),
        }
    }

    /// Runs `coffret` with no Passphrase on standard input.
    fn run(&self, arguments: &[&str]) -> Output {
        self.run_with(arguments, None)
    }

    /// Runs `coffret`, offering `passphrase` on standard input.
    fn run_with(&self, arguments: &[&str], passphrase: Option<&str>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_coffret"))
            .args(arguments)
            .env("COFFRET_STATE_DIR", self.state.path())
            // The logs go to the same throwaway directory, so a run leaves
            // nothing in the state directory of whoever started it.
            .env("COFFRET_LOG_DIR", self.state.path().join("logs"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the built binary must be runnable");

        let mut stdin = child.stdin.take().expect("standard input was piped");
        if let Some(passphrase) = passphrase {
            writeln!(stdin, "{passphrase}").expect("the Passphrase must be writable");
        }
        drop(stdin);

        child.wait_with_output().expect("the run must finish")
    }

    /// The folder at `name`, created.
    fn folder(&self, name: &str) -> PathBuf {
        let path = self.folders.path().join(name);
        std::fs::create_dir_all(&path).expect("the folder must be creatable");
        path
    }
}

/// What a run wrote to standard output.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// The Recovery Code a run printed, as one line.
fn printed_code(output: &Output) -> String {
    stdout(output)
        .lines()
        .find(|line| line.starts_with(RECOVERY_CODE_PREFIX))
        .unwrap_or_else(|| {
            panic!(
                "no Recovery Code on standard output; stderr was:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        })
        .to_owned()
}

/// Creates an S3 Library called `name` on `device`.
fn init_s3(device: &Device, name: &str) -> Output {
    let output = device.run_with(
        &[
            "init",
            "--name",
            name,
            "--s3",
            "--bucket",
            "photos",
            "--prefix",
            "archive/",
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    assert!(
        output.status.success(),
        "init must succeed; stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

// The whole of what a person does to get a Library going: create it, map a
// folder, and see the mapping listed back.
#[test]
fn a_library_is_created_mapped_and_listed() {
    let device = Device::new();
    let created = init_s3(&device, "alpha");

    // The Recovery Code is on standard output, so it can be piped somewhere
    // safe; everything around it is on standard error.
    let code = printed_code(&created);
    assert!(code.starts_with(RECOVERY_CODE_PREFIX), "{code}");

    let albums = device.folder("albums");
    let mapped = device.run(&[
        "map",
        "--library",
        "alpha",
        "--prefix",
        "albums",
        albums.to_str().expect("the folder has a usable name"),
    ]);
    assert!(mapped.status.success());

    let listed = device.run(&["mappings", "--library", "alpha"]);
    assert!(listed.status.success());
    let listed = stdout(&listed);
    assert!(
        listed.contains("albums\t") && listed.contains(albums.to_str().unwrap()),
        "the mapping must be listed: {listed:?}"
    );
}

// The code is written out again from the stored form rather than kept, so
// asking for it again with the Passphrase yields the same one.
#[test]
fn the_recovery_code_is_printed_again_for_whoever_knows_the_passphrase() {
    let device = Device::new();
    let first = printed_code(&init_s3(&device, "again"));

    let output = device.run_with(
        &["recovery-code", "--library", "again", "--passphrase-stdin"],
        Some(PASSPHRASE),
    );
    assert!(output.status.success());
    assert_eq!(printed_code(&output), first);
}

// DK-5: a Passphrase that does not open the stored form yields no code rather
// than a different one, and the run says so by failing.
#[test]
fn a_wrong_passphrase_prints_no_recovery_code() {
    let device = Device::new();
    init_s3(&device, "guarded");

    let output = device.run_with(
        &[
            "recovery-code",
            "--library",
            "guarded",
            "--passphrase-stdin",
        ],
        Some("not the Passphrase"),
    );

    assert!(
        !output.status.success(),
        "a wrong Passphrase must not succeed"
    );
    assert!(
        !stdout(&output).contains(RECOVERY_CODE_PREFIX),
        "nothing that looks like a code may be printed: {:?}",
        stdout(&output)
    );
}

// A second `init` would strand whatever the first put on Storage, and the shell
// has to report that as a failed run rather than as a line of prose.
#[test]
fn a_second_library_of_one_name_fails_the_run() {
    let device = Device::new();
    init_s3(&device, "twice");

    let output = device.run_with(
        &[
            "init",
            "--name",
            "twice",
            "--s3",
            "--bucket",
            "photos",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );

    assert!(!output.status.success());
    assert!(!stdout(&output).contains(RECOVERY_CODE_PREFIX));
}

// The prompt refuses an empty Passphrase, and so must the line a script pipes:
// otherwise the one way of creating a Library that nobody watches is the one
// that would store a Master Key protected by nothing.
#[test]
fn an_empty_passphrase_from_a_script_creates_nothing() {
    let device = Device::new();

    let output = device.run_with(
        &[
            "init",
            "--name",
            "unprotected",
            "--s3",
            "--bucket",
            "photos",
            "--passphrase-stdin",
        ],
        Some(""),
    );

    assert!(
        !output.status.success(),
        "an empty Passphrase must not create a Library"
    );
    assert!(!stdout(&output).contains(RECOVERY_CODE_PREFIX));
    assert!(!libraries(&device).join("unprotected").exists());
}

// The flags say where the Library goes, and exactly one of them has to.
#[test]
fn a_provider_has_to_be_named_and_only_one_of_them() {
    let device = Device::new();

    for arguments in [
        vec!["init", "--name", "nowhere", "--passphrase-stdin"],
        vec![
            "init",
            "--name",
            "everywhere",
            "--drive",
            "--s3",
            "--bucket",
            "photos",
            "--passphrase-stdin",
        ],
        // `--s3` without a bucket names no bucket to put it in.
        vec!["init", "--name", "unbucketed", "--s3", "--passphrase-stdin"],
        // A flag the chosen provider knows nothing about is refused rather
        // than ignored: accepting this one would look like the Library had
        // been put at that endpoint.
        vec![
            "init",
            "--name",
            "confused",
            "--drive",
            "--endpoint",
            "http://127.0.0.1:19000",
            "--passphrase-stdin",
        ],
        vec![
            "init",
            "--name",
            "confused",
            "--s3",
            "--bucket",
            "photos",
            "--parent",
            "1a2B3c",
            "--passphrase-stdin",
        ],
    ] {
        let output = device.run_with(&arguments, Some(PASSPHRASE));
        assert!(
            !output.status.success(),
            "{arguments:?} must not create a Library"
        );
    }

    assert!(!libraries(&device).exists() || libraries(&device).read_dir().unwrap().count() == 0);
}

/// Where Libraries would be on a device.
fn libraries(device: &Device) -> PathBuf {
    device.state.path().join("libraries")
}
