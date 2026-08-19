//! What the exchange does when the bytes it is handed are not the bytes that
//! were written.
//!
//! The exchange only means something if it fails when it should. Editing one
//! ciphertext byte of a fixture is the cheapest corruption there is, and the
//! run has to stop on it and say which fixture it stopped on — a verifier that
//! passed here would pass on a genuine incompatibility too.

use std::fs;
use std::path::Path;

use coffret_interop::manifest::{Manifest, MANIFEST_FILE};

/// The fixture whose bytes each case edits.
const FIXTURE: &str = "multi-entry";

#[test]
fn an_intact_set_verifies() {
    let directory = tempfile::tempdir().expect("a temporary directory is available");
    coffret_interop::generate(directory.path()).expect("the set is generated");
    coffret_interop::verify(directory.path()).expect("a set this build wrote verifies");
}

#[test]
fn a_flipped_ciphertext_byte_fails_and_names_the_fixture() {
    let directory = tempfile::tempdir().expect("a temporary directory is available");
    coffret_interop::generate(directory.path()).expect("the set is generated");

    // The last byte of the object is inside the final chunk's tag, so flipping
    // it is a corruption no reader may release plaintext past (FM-1).
    let path = fixture_path(directory.path(), FIXTURE);
    let mut bytes = fs::read(&path).expect("the fixture is readable");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(&path, &bytes).expect("the fixture is writable");

    let error = coffret_interop::verify(directory.path()).expect_err("the fixture is corrupted");
    let report = format!("{error:#}");
    // Both directions of the exchange are verified by this same code, so the
    // report has to say which side wrote the set it stopped on.
    assert!(report.contains(r#""rust""#), "{report}");
    assert!(report.contains(FIXTURE), "{report}");
    assert!(report.contains("authentication"), "{report}");
}

#[test]
fn an_edited_expectation_fails_and_names_the_field() {
    let directory = tempfile::tempdir().expect("a temporary directory is available");
    coffret_interop::generate(directory.path()).expect("the set is generated");

    // A manifest that states an Entry path the object does not carry is the
    // same failure a genuine decoder disagreement would produce.
    let path = directory.path().join(MANIFEST_FILE);
    let mut manifest = read_manifest(&path);
    manifest
        .containers
        .iter_mut()
        .find(|fixture| fixture.fixture == FIXTURE)
        .expect("the fixture is listed")
        .entries[0]
        .path = "album/2019/somewhere-else.jpg".to_owned();
    fs::write(
        &path,
        serde_json::to_string(&manifest).expect("it serializes"),
    )
    .expect("the manifest is writable");

    let error = coffret_interop::verify(directory.path()).expect_err("the manifest disagrees");
    let report = format!("{error:#}");
    assert!(report.contains(FIXTURE), "{report}");
    assert!(report.contains("path"), "{report}");
}

#[test]
fn a_set_missing_a_fixture_fails_before_anything_is_opened() {
    let directory = tempfile::tempdir().expect("a temporary directory is available");
    coffret_interop::generate(directory.path()).expect("the set is generated");

    let path = directory.path().join(MANIFEST_FILE);
    let mut manifest = read_manifest(&path);
    manifest
        .control_objects
        .retain(|fixture| fixture.fixture != "index-snapshot");
    fs::write(
        &path,
        serde_json::to_string(&manifest).expect("it serializes"),
    )
    .expect("the manifest is writable");

    let error = coffret_interop::verify(directory.path()).expect_err("a kind is missing");
    assert!(format!("{error:#}").contains("index-snapshot"), "{error:#}");
}

fn read_manifest(path: &Path) -> Manifest {
    let json = fs::read_to_string(path).expect("the manifest is readable");
    serde_json::from_str(&json).expect("the manifest parses")
}

fn fixture_path(root: &Path, fixture: &str) -> std::path::PathBuf {
    let manifest = read_manifest(&root.join(MANIFEST_FILE));
    let file = manifest
        .containers
        .iter()
        .find(|candidate| candidate.fixture == fixture)
        .expect("the fixture is listed")
        .file
        .clone();
    root.join(file)
}
