//! The one collation this adapter may use, checked against its own source.
//!
//! Entry Path equality is exact equality of the canonical bytes and is
//! case-sensitive; NFC merges neither case nor width variants (spec: EP-3). A
//! catalog that compared case-insensitively would answer one of the user's
//! files with another's Entry, and would do it silently — the conformance suite
//! catches it for the columns it exercises, but the mistake is one character
//! long and could land on a column no case reads yet.
//!
//! So the source is checked rather than only the behaviour: every path column
//! says `COLLATE BINARY`, and the word `NOCASE` appears nowhere in this crate.

use std::fs;
use std::path::{Path, PathBuf};

/// The collation every Entry Path column is compared with.
const REQUIRED: &str = "COLLATE BINARY";

/// The collation that would fold two of the user's files into one.
///
/// Spelled in halves so that this file, which the walk deliberately does not
/// read, could not be the thing that fails the check if it ever were read.
const FORBIDDEN: [&str; 2] = ["NO", "CASE"];

/// Every `.rs` file under the crate's `src`.
fn sources() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory).expect("the crate's own sources must be readable");
        for entry in entries {
            let path = entry.expect("a directory entry must be readable").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
    }
    assert!(!found.is_empty(), "the crate has sources to check");
    found
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).expect("a source file must be readable")
}

#[test]
fn no_column_is_compared_case_insensitively() {
    let forbidden = FORBIDDEN.concat();
    for path in sources() {
        assert!(
            !read(&path).contains(&forbidden),
            "{} must not use {forbidden}: Entry Paths are compared bytewise (spec: EP-3)",
            path.display()
        );
    }
}

#[test]
fn every_path_column_is_compared_bytewise() {
    let schema = read(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/schema.rs"));
    let path_columns = schema
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("path ")
                || line.starts_with("prefix ")
                || line.starts_with("derived_from_path ")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        path_columns.len(),
        4,
        "every Entry Path column is accounted for, found {path_columns:?}"
    );
    for column in path_columns {
        assert!(
            column.contains(REQUIRED),
            "{column:?} must say {REQUIRED} (spec: EP-3)"
        );
    }
}
