//! `scripts/rename-smoke.sh` against a Library on MinIO.
//!
//! The script is what a person runs against a Library of their own on real
//! Storage to see that the build in their checkout renames correctly. Nothing
//! but a person runs it there, so a script that had drifted from what the CLI
//! answers would wait for the next person to find it. These cases run it where
//! the round trip runs, against a Library made the way the round trip makes
//! one, with the Passphrase on its standard input as `read -rs` takes it from a
//! pipe, and the binary cargo built for the test in place of the release build
//! it would otherwise make.
//!
//! Without MinIO the cases report themselves skipped, as the round trip's do.
//! The script is bash, so the file is compiled where bash is the shell.

#![cfg(unix)]

mod support;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

use support::{code, stderr, stdout, succeeded, Device, Minio, PASSPHRASE, REGION};

/// The script under test, from the repository root.
fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../scripts/rename-smoke.sh")
}

// The whole run, under the Library-root mapping, which the script prefers to
// the prefixed one mapped beside it.
#[tokio::test]
async fn the_smoke_test_passes_under_the_library_root() {
    let Some(minio) = support::minio("rename-smoke-root") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    minio.ensure_bucket().await;
    let device = Device::new();
    init(&device, "smoke", &minio);
    let root = device.folder("root");
    let books = device.folder("books");
    map(&device, "smoke", None, &root);
    map(&device, "smoke", Some("books"), &books);

    let output = run_script(&device, "smoke", PASSPHRASE);
    passed(&output);
    assert_only_smoke_folder(&root);
    assert!(
        listed(&books).iter().all(|name| name == ".coffret"),
        "the prefixed mapping beside the root one is left alone: {:?}",
        listed(&books)
    );
}

// The whole run, under the first mapping where the Library root is not mapped,
// which puts the smoke folder's Entry Path under that mapping's prefix.
#[tokio::test]
async fn the_smoke_test_passes_under_a_prefixed_mapping() {
    let Some(minio) = support::minio("rename-smoke-prefix") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    minio.ensure_bucket().await;
    let device = Device::new();
    init(&device, "smoke", &minio);
    let books = device.folder("books");
    map(&device, "smoke", Some("books"), &books);

    let output = run_script(&device, "smoke", PASSPHRASE);
    passed(&output);
    assert!(
        stderr(&output).contains("stays in the Library: books/rename-smoke-"),
        "the smoke folder's Entry Path is under the mapping's prefix: {}",
        stderr(&output)
    );
    assert_only_smoke_folder(&books);
}

// A Library with nothing mapped has nowhere to put the smoke folder.
#[tokio::test]
async fn a_library_with_no_mapping_is_refused() {
    let Some(minio) = support::minio("rename-smoke-unmapped") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    minio.ensure_bucket().await;
    let device = Device::new();
    init(&device, "smoke", &minio);

    let output = run_script(&device, "smoke", PASSPHRASE);
    assert_eq!(code(&output), 1, "stderr was:\n{}", stderr(&output));
    assert!(
        stderr(&output).contains("has no mappings"),
        "the refusal says why: {}",
        stderr(&output)
    );
    never_shows_the_passphrase(&output, PASSPHRASE);
}

// A wrong Passphrase is refused by the first sync, before the smoke folder is
// written.
#[tokio::test]
async fn a_wrong_passphrase_is_refused_before_anything_is_written() {
    let Some(minio) = support::minio("rename-smoke-passphrase") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    minio.ensure_bucket().await;
    let device = Device::new();
    init(&device, "smoke", &minio);
    let root = device.folder("root");
    map(&device, "smoke", None, &root);

    let wrong = "not the Passphrase at all";
    let output = run_script(&device, "smoke", wrong);
    assert_eq!(code(&output), 1, "stderr was:\n{}", stderr(&output));
    assert!(
        stderr(&output).contains("the first sync failed"),
        "the refusal names the step: {}",
        stderr(&output)
    );
    assert!(
        !stderr(&output).contains("stays in the Library"),
        "no smoke folder was made, so none is reported"
    );
    never_shows_the_passphrase(&output, wrong);
    assert!(
        listed(&root).iter().all(|name| name == ".coffret"),
        "nothing is written before the Passphrase is accepted: {:?}",
        listed(&root)
    );
}

// A check that does not hold fails the run, names the check, and still says
// where the smoke folder was left. The binary is wrapped so that `rename`
// answers with a count it did not rename, which no real build does on purpose.
#[tokio::test]
async fn a_check_that_does_not_hold_fails_the_run() {
    let Some(minio) = support::minio("rename-smoke-check") else {
        eprintln!("skipped: no S3 implementation is configured");
        return;
    };
    minio.ensure_bucket().await;
    let device = Device::new();
    init(&device, "smoke", &minio);
    let root = device.folder("root");
    map(&device, "smoke", None, &root);

    let wrapper_dir = TempDir::new().expect("a temporary directory must be available");
    let wrapper = wrapper_dir.path().join("coffret");
    std::fs::write(
        &wrapper,
        format!(
            "#!/usr/bin/env bash\n\
             set -o pipefail\n\
             '{}' \"$@\" | jq -c 'if .command == \"rename\" then .answer.renamed = 99 else . end'\n",
            env!("CARGO_BIN_EXE_coffret")
        ),
    )
    .expect("the wrapper must be writable");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))
        .expect("the wrapper must be executable");

    let output = run_script_with(&device, "smoke", PASSPHRASE, &wrapper);
    assert_eq!(code(&output), 1, "stderr was:\n{}", stderr(&output));
    assert!(
        stderr(&output).contains("FAILED: coffret rename: expected .answer.renamed to be 3"),
        "the failure names the check: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("delete it from the explorer"),
        "a failed run still says where the smoke folder is: {}",
        stderr(&output)
    );
    never_shows_the_passphrase(&output, PASSPHRASE);
}

/// Runs the script for `library` on `device`, with `passphrase` on its standard
/// input.
fn run_script(device: &Device, library: &str, passphrase: &str) -> Output {
    run_script_with(
        device,
        library,
        passphrase,
        Path::new(env!("CARGO_BIN_EXE_coffret")),
    )
}

/// Runs the script as [`run_script`] does, with `binary` as the CLI.
fn run_script_with(device: &Device, library: &str, passphrase: &str, binary: &Path) -> Output {
    let mut child = Command::new(script())
        .arg(library)
        .env("COFFRET_STATE_DIR", device.state())
        .env("COFFRET_LOG_DIR", device.state().join("logs"))
        .env("COFFRET_RENAME_SMOKE_BIN", binary)
        // For the reason `Device::run_with` takes them out: the script needs
        // neither, and a machine set up for the Drive targets has both.
        .env_remove("COFFRET_DRIVE_CLIENT_ID")
        .env_remove("COFFRET_DRIVE_CLIENT_SECRET")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the script must be runnable");

    let mut stdin = child.stdin.take().expect("standard input was piped");
    writeln!(stdin, "{passphrase}").expect("the script reads its standard input");
    drop(stdin);

    child.wait_with_output().expect("the script must finish")
}

/// Asserts that a run of the script passed and said what it leaves behind.
fn passed(output: &Output) {
    succeeded(output, "rename-smoke.sh");
    assert!(
        stdout(output).contains("rename-smoke: passed against smoke"),
        "the run says it passed: {}",
        stdout(output)
    );
    assert!(
        stderr(output).contains("delete it from the explorer"),
        "the run says the smoke folder is left in the Library: {}",
        stderr(output)
    );
    never_shows_the_passphrase(output, PASSPHRASE);
}

fn never_shows_the_passphrase(output: &Output, passphrase: &str) {
    assert!(!stdout(output).contains(passphrase));
    assert!(!stderr(output).contains(passphrase));
}

/// Asserts that `root` holds coffret's management area and one smoke folder,
/// with the books as the run leaves them.
fn assert_only_smoke_folder(root: &Path) {
    let names = listed(root);
    let smoke: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with("rename-smoke-"))
        .collect();
    assert_eq!(smoke.len(), 1, "one smoke folder: {names:?}");
    assert!(
        names
            .iter()
            .all(|name| name == ".coffret" || name.starts_with("rename-smoke-")),
        "nothing is written outside the smoke folder: {names:?}"
    );

    let smoke = root.join(smoke[0]);
    for page in ["page-01.svg", "page-02.svg", "page-03.svg"] {
        assert!(smoke.join("renamed/book-a").join(page).is_file());
    }
    assert!(smoke.join("book-b/cover.svg").is_file());
    assert!(smoke.join("book-b/extra/page-02.svg").is_file());
    assert!(smoke.join("book-b/page-03.svg").is_file());
}

/// The names directly under `folder`.
fn listed(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .expect("the folder must be readable")
        .map(|entry| {
            entry
                .expect("the entry must be readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Creates the Library `name` in the bucket.
fn init(device: &Device, name: &str, minio: &Minio) {
    let output = device.run_with(
        &[
            "init",
            "--name",
            name,
            "--s3",
            "--bucket",
            &minio.bucket,
            "--prefix",
            &minio.base_prefix,
            "--endpoint",
            &minio.endpoint,
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&output, "init");
}

/// Maps `local_root` at `prefix`, or at the Library root where it is `None`.
fn map(device: &Device, name: &str, prefix: Option<&str>, local_root: &Path) {
    let local_root = local_root.to_str().expect("the folder has a usable name");
    let mut arguments = vec!["map", "--library", name];
    if let Some(prefix) = prefix {
        arguments.extend(["--prefix", prefix]);
    }
    arguments.push(local_root);
    succeeded(&device.run(&arguments), "map");
}
