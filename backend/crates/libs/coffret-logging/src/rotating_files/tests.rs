//! What the ceiling is worth: that it holds, and that it costs the oldest
//! evidence rather than the newest.

use std::fs;
use std::io::Write;

use tempfile::TempDir;
use tracing_subscriber::fmt::MakeWriter;

use super::files::MAX_FILES;
use super::RotatingFiles;
use crate::log_settings::LogSettings;

/// A budget small enough to overrun in a test, and large enough to hold events.
const CEILING: u64 = 8 * 1024;
/// Four files' worth of it, so pruning has something to choose between.
const PER_FILE: u64 = 2 * 1024;

/// A sink over a directory that goes away with the test.
fn sink(directory: &TempDir) -> RotatingFiles {
    let settings = LogSettings::new(directory.path())
        .with_ceiling(CEILING)
        .with_max_file_bytes(PER_FILE);

    RotatingFiles::open(&settings).expect("a temporary directory must be writable")
}

/// Writes one record, the way the formatting layer does.
fn write(files: &RotatingFiles, record: &str) {
    files
        .make_writer()
        .write_all(record.as_bytes())
        .expect("a temporary directory must be writable");
}

/// Everything currently on disk in the directory.
fn on_disk(directory: &TempDir) -> Vec<(String, u64)> {
    let mut found: Vec<_> = fs::read_dir(directory.path())
        .expect("the directory must be readable")
        .map(|entry| entry.expect("each entry must be readable"))
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.metadata().expect("each file must be readable").len(),
            )
        })
        .collect();

    found.sort();
    found
}

/// Everything currently on disk, as one string.
fn contents(directory: &TempDir) -> String {
    on_disk(directory)
        .into_iter()
        .map(|(name, _)| {
            fs::read_to_string(directory.path().join(name)).expect("a log file must be readable")
        })
        .collect()
}

/// The total bytes the directory holds.
fn total_bytes(directory: &TempDir) -> u64 {
    on_disk(directory).into_iter().map(|(_, len)| len).sum()
}

#[test]
fn writing_far_past_the_ceiling_leaves_the_ceiling_intact() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    let files = sink(&directory);

    // Twenty times the budget, written a hundred bytes at a time.
    for record in 0..2000 {
        write(&files, &format!("{:0>98}\n", format!("record-{record}")));
        assert!(
            total_bytes(&directory) <= CEILING,
            "the ceiling was passed after {record} records: {} bytes",
            total_bytes(&directory),
        );
    }
}

#[test]
fn the_oldest_evidence_is_what_the_ceiling_costs() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    let files = sink(&directory);

    for record in 0..2000 {
        write(&files, &format!("{:0>98}\n", format!("record-{record}")));
    }

    let kept = contents(&directory);
    assert!(
        !kept.contains("record-0\n"),
        "the first record should have been dropped",
    );
    assert!(
        kept.contains("record-1999"),
        "the newest record must still be there",
    );
}

#[test]
fn a_record_larger_than_a_file_is_cut_down_rather_than_breaking_the_budget() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    let files = sink(&directory);

    write(&files, &format!("{}\n", "e".repeat(PER_FILE as usize * 4)));

    assert!(total_bytes(&directory) <= CEILING);
    let kept = contents(&directory);
    assert!(kept.contains("[record truncated]"));
    assert!(kept.starts_with("eeee"));
}

#[test]
fn a_multibyte_character_is_never_cut_in_half() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    let files = sink(&directory);

    // Three bytes per character, so a cut at an arbitrary byte lands inside one.
    write(&files, &format!("{}\n", "あ".repeat(PER_FILE as usize)));

    let kept = contents(&directory);
    assert!(kept.ends_with("[record truncated]\n"));
}

#[test]
fn files_left_by_an_earlier_run_count_against_the_ceiling() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    for run in 0..8 {
        fs::write(
            directory
                .path()
                .join(format!("coffret-20260101T00000{run}-000.log")),
            vec![b'x'; PER_FILE as usize],
        )
        .expect("a temporary directory must be writable");
    }

    let files = sink(&directory);
    write(&files, "the run that follows them\n");

    assert!(total_bytes(&directory) <= CEILING);
    assert!(contents(&directory).contains("the run that follows them"));
}

#[test]
fn a_program_started_over_and_over_does_not_fill_the_directory() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");

    // A run that emits nothing leaves an empty file, which weighs nothing and
    // so is never what the byte ceiling prunes.
    for _ in 0..MAX_FILES * 3 {
        let _ = sink(&directory);
    }

    let kept = on_disk(&directory).len();
    assert!(kept <= MAX_FILES, "{kept} files were left behind");
}

#[test]
fn nothing_else_in_the_directory_is_deleted() {
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    let bystander = directory.path().join("notes.txt");
    fs::write(&bystander, "not a log file").expect("a temporary directory must be writable");

    let files = sink(&directory);
    for record in 0..2000 {
        write(&files, &format!("{:0>98}\n", format!("record-{record}")));
    }

    assert!(bystander.exists(), "a file that is not a log was deleted");
}

#[cfg(unix)]
#[test]
fn a_log_file_is_readable_by_its_owner_and_by_nobody_else() {
    use std::os::unix::fs::PermissionsExt;

    use super::create_directory::OWNER_ONLY_DIRECTORY;
    use super::start_file::OWNER_ONLY_FILE;

    let parent = TempDir::new().expect("a temporary directory must be creatable");
    // A directory that is not there yet, so the mode asserted below is the one
    // this crate gave it rather than one the temporary directory came with.
    let directory = parent.path().join("state/coffret/logs");
    let files = RotatingFiles::open(&LogSettings::new(&directory))
        .expect("a temporary directory must be writable");
    files
        .make_writer()
        .write_all(b"one event\n")
        .expect("a temporary directory must be writable");

    let path = files.current_path();
    let mode = fs::metadata(&path)
        .expect("the log file must exist")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, OWNER_ONLY_FILE, "{path:?} is not owner-only");

    let mode = fs::metadata(&directory)
        .expect("the directory must exist")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, OWNER_ONLY_DIRECTORY, "{directory:?}");
}

#[cfg(unix)]
#[test]
fn a_directory_that_was_already_there_keeps_the_permissions_it_had() {
    use std::os::unix::fs::PermissionsExt;

    // `COFFRET_LOG_DIR` may name a directory that is somebody else's — a home,
    // a shared temporary directory — and logging into one is no reason to
    // change what anything else may do with it.
    let directory = TempDir::new().expect("a temporary directory must be creatable");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755))
        .expect("a temporary directory must be writable");

    let _files = sink(&directory);

    let mode = fs::metadata(directory.path())
        .expect("the directory must exist")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755, "the directory's permissions changed");
}
