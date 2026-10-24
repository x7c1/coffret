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
use coffret_interop::{Error, FixtureKind, Step};

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
    // it is a corruption no reader may release plaintext past (spec: FM-1).
    let path = fixture_path(directory.path(), FIXTURE);
    let mut bytes = fs::read(&path).expect("the fixture is readable");
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    fs::write(&path, &bytes).expect("the fixture is writable");

    let error = coffret_interop::verify(directory.path()).expect_err("the fixture is corrupted");
    let report = report(&error);
    // Both directions of the exchange are verified by this same code, so the
    // failure has to say which side wrote the set it stopped on.
    let failure = in_fixture(error, "rust", FixtureKind::Container, FIXTURE);
    assert!(
        matches!(
            failure,
            Error::Format {
                step: Step::OpeningContainer,
                ..
            }
        ),
        "{report}"
    );
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
    let report = report(&error);
    let failure = in_fixture(error, "rust", FixtureKind::Container, FIXTURE);
    let Error::InEntry { index: 0, cause } = failure else {
        panic!("the first Entry is the one edited: {report}");
    };
    assert!(
        matches!(&*cause, Error::Differs { field, .. } if field == "path"),
        "{report}"
    );
}

#[test]
fn a_mistyped_recovery_code_fails_and_names_the_fixture() {
    let directory = tempfile::tempdir().expect("a temporary directory is available");
    coffret_interop::generate(directory.path()).expect("the set is generated");

    // One character wrong is what a hand copy gets wrong, and it is exactly what
    // the checksum is there to catch (spec: KD-11) — so the exchange has to stop
    // on it rather than read the code as some other Master Key.
    let path = recovery_code_path(directory.path());
    let code = fs::read_to_string(&path).expect("the code is readable");
    let at = 30;
    let replacement = if code.as_bytes()[at] == b'q' {
        'p'
    } else {
        'q'
    };
    let mistyped = format!("{}{replacement}{}", &code[..at], &code[at + 1..]);
    fs::write(&path, mistyped).expect("the code is writable");

    let error = coffret_interop::verify(directory.path()).expect_err("the code is mistyped");
    let report = report(&error);
    assert_parsing_refused(error, &report);
    assert!(report.contains("checksum"), "{report}");
}

/// A code that does not divide into a prefix and a data part, with the words
/// both readers answer it in: the TypeScript reader's tests hold it to the same
/// three answers, so a verifier that folded them back into one fails here.
const UNDIVIDED_CODES: [(&str, &str); 3] = [
    ("1qqqqqqq", "nothing before its separator"),
    ("coffretqqqq", "no separator"),
    ("c\u{f6}ffret1qqqqqqqq", "prefix holds no character"),
];

#[test]
fn a_code_that_does_not_divide_fails_naming_the_check_and_the_fixture() {
    for (code, expected) in UNDIVIDED_CODES {
        let directory = tempfile::tempdir().expect("a temporary directory is available");
        coffret_interop::generate(directory.path()).expect("the set is generated");
        fs::write(recovery_code_path(directory.path()), code).expect("the code is writable");

        let error = coffret_interop::verify(directory.path()).expect_err("the code is malformed");
        let report = report(&error);
        assert_parsing_refused(error, &report);
        assert!(report.contains(expected), "{code:?}: {report}");
    }
}

/// A code pasted twice divides at the second copy's separator, under a prefix
/// longer than Bech32 lets one be. The checksum, which KD-11 checks before the
/// prefix, ends the read without quoting the first copy; the TypeScript
/// reader's tests hold it to the same answer.
#[test]
fn a_code_pasted_twice_fails_the_checksum_without_quoting_itself() {
    let directory = tempfile::tempdir().expect("a temporary directory is available");
    coffret_interop::generate(directory.path()).expect("the set is generated");
    let path = recovery_code_path(directory.path());
    let code = fs::read_to_string(&path).expect("the code is readable");
    let code = code.trim();
    fs::write(&path, format!("{code}{code}")).expect("the code is writable");

    let error = coffret_interop::verify(directory.path()).expect_err("the code is doubled");
    let report = report(&error);
    assert_parsing_refused(error, &report);
    assert!(report.contains("checksum"), "{report}");
    assert!(!report.contains(&code["coffret1".len()..]), "{report}");
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
    assert!(
        matches!(
            error,
            Error::FixtureUnlisted {
                kind: FixtureKind::ControlObject,
                name: "index-snapshot",
            }
        ),
        "{error:?}"
    );
}

/// The whole chain, link by link: what the format crate itself answered is the
/// innermost link, and its words are what both readers are held to.
fn report(error: &Error) -> String {
    error.chain().join(": ")
}

/// The failure met in the fixture `fixture` of the set `producer` wrote,
/// insisting that is where the run stopped.
fn in_fixture(error: Error, producer: &str, kind: FixtureKind, fixture: &str) -> Error {
    let report = report(&error);
    let Error::InSet {
        producer: wrote,
        cause,
    } = error
    else {
        panic!("the failure names the set it was met in: {report}");
    };
    assert_eq!(wrote, producer, "{report}");
    let Error::InFixture {
        kind: met_in,
        fixture: name,
        cause,
    } = *cause
    else {
        panic!("the failure names the fixture it was met at: {report}");
    };
    assert_eq!((met_in, name.as_str()), (kind, fixture), "{report}");
    *cause
}

/// Insists the run stopped on the format crate refusing the bare Recovery
/// Code this side wrote.
fn assert_parsing_refused(error: Error, report: &str) {
    let failure = in_fixture(error, "rust", FixtureKind::RecoveryCode, "recovery-code");
    assert!(
        matches!(
            failure,
            Error::Format {
                step: Step::ParsingRecoveryCode,
                ..
            }
        ),
        "{report}"
    );
}

fn read_manifest(path: &Path) -> Manifest {
    let json = fs::read_to_string(path).expect("the manifest is readable");
    serde_json::from_str(&json).expect("the manifest parses")
}

/// Where the bare Recovery Code of a set lives, as its manifest points at it.
fn recovery_code_path(root: &Path) -> std::path::PathBuf {
    let file = read_manifest(&root.join(MANIFEST_FILE))
        .recovery_codes
        .iter()
        .find(|candidate| candidate.fixture == "recovery-code")
        .expect("the fixture is listed")
        .file
        .clone();
    root.join(file)
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
