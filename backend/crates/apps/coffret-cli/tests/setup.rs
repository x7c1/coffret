//! Setting a Library up from the command line, as a person does it.
//!
//! The cases here create S3 Libraries, which write nothing to Storage — a prefix
//! exists by being written under, so nothing is there until the first commit —
//! and ask it one question: whether the bucket is there at all. That question is
//! answered by `support::stub_endpoint`, so the whole of `init`, `join`, `map`,
//! `mappings` and `recovery-code` is exercised in an ordinary test run. What a
//! real implementation answers is the round trip's business.

mod support;

use std::fs;
use std::path::Path;
use std::process::Output;

use support::{
    code, printed_code, printed_prefix, stderr, stdout, stub_endpoint, succeeded, Device,
    PASSPHRASE, RECOVERY_CODE_PREFIX, REGION,
};

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
            "--endpoint",
            stub_endpoint(),
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&output, "init");
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
    let printed = printed_code(&created);
    assert!(printed.starts_with(RECOVERY_CODE_PREFIX), "{printed}");

    let albums = device.folder("albums");
    let mapped = device.run(&[
        "map",
        "--library",
        "alpha",
        "--prefix",
        "albums",
        albums.to_str().expect("the folder has a usable name"),
    ]);
    succeeded(&mapped, "map");

    let listed = device.run(&["mappings", "--library", "alpha"]);
    succeeded(&listed, "mappings");
    let listed = stdout(&listed);
    assert!(
        listed.contains("albums\t") && listed.contains(albums.to_str().unwrap()),
        "the mapping must be listed: {listed:?}"
    );
}

// Moving a mapping takes everything under the old root out of the Library's
// reach on this device, so a person who typed the wrong prefix has to be told
// what they just moved rather than left to find out.
#[test]
fn remapping_a_prefix_says_what_it_was_at() {
    let device = Device::new();
    init_s3(&device, "moving");
    let first = device.folder("albums");
    let second = device.folder("albums-moved");

    let mapped = device.run(&[
        "map",
        "--library",
        "moving",
        "--prefix",
        "albums",
        first.to_str().unwrap(),
    ]);
    succeeded(&mapped, "map");
    assert!(
        !stderr(&mapped).contains("was at"),
        "the first mapping replaced nothing: {:?}",
        stderr(&mapped)
    );

    let moved = device.run(&[
        "map",
        "--library",
        "moving",
        "--prefix",
        "albums",
        second.to_str().unwrap(),
    ]);
    succeeded(&moved, "map");
    let said = stderr(&moved);
    assert!(
        said.contains(&format!(
            "albums was at {}; it is now at {}.",
            first.canonicalize().unwrap().display(),
            second.canonicalize().unwrap().display()
        )),
        "moving a mapping must say what it moved: {said:?}"
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
    succeeded(&output, "recovery-code");
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

    assert_eq!(code(&output), 1, "a wrong Passphrase must not succeed");
    assert!(
        !stdout(&output).contains(RECOVERY_CODE_PREFIX),
        "nothing that looks like a code may be printed: {:?}",
        stdout(&output)
    );
}

// A second `init` would strand whatever the first put on Storage, and the shell
// has to report that as a failed run rather than as a line of prose.
//
// Refused with nothing on standard input, which is the point of the case: the
// Passphrase is asked for through a callback the device layer only reaches once
// every refusal that needs no key has passed, so a script whose Library already
// exists is told that rather than told that its empty Passphrase protects
// nothing.
#[test]
fn a_second_library_of_one_name_is_refused_before_a_passphrase_is_read() {
    let device = Device::new();
    init_s3(&device, "twice");

    let output = device.run(&[
        "init",
        "--name",
        "twice",
        "--s3",
        "--bucket",
        "photos",
        "--endpoint",
        stub_endpoint(),
        "--region",
        REGION,
        "--path-style",
        "--passphrase-stdin",
    ]);

    assert_eq!(code(&output), 1);
    assert!(!stdout(&output).contains(RECOVERY_CODE_PREFIX));
    let said = stderr(&output);
    assert!(
        said.contains("already at"),
        "the refusal must be the one about the Library, not about the Passphrase: {said:?}"
    );
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
            "--endpoint",
            stub_endpoint(),
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(""),
    );

    assert_eq!(
        code(&output),
        1,
        "an empty Passphrase must not create a Library"
    );
    assert!(!stdout(&output).contains(RECOVERY_CODE_PREFIX));
    assert!(!device.libraries().join("unprotected").exists());
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
            "--parent",
            "1a2B3c",
            "--s3",
            "--bucket",
            "photos",
            "--passphrase-stdin",
        ],
        // `--s3` without a bucket names no bucket to put it in.
        vec!["init", "--name", "unbucketed", "--s3", "--passphrase-stdin"],
        // A Drive Library has to be told which folder to go in: the top of My
        // Drive is never what was meant, and it is what Drive does with a
        // create that names no parent.
        vec![
            "init",
            "--name",
            "unparented",
            "--drive",
            "--client-id",
            "someone.apps.googleusercontent.com",
            "--passphrase-stdin",
        ],
        // A flag the chosen provider knows nothing about is refused rather
        // than ignored: accepting this one would look like the Library had
        // been put at that endpoint.
        vec![
            "init",
            "--name",
            "confused",
            "--drive",
            "--parent",
            "1a2B3c",
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
        assert_eq!(
            code(&output),
            1,
            "{arguments:?} must not create a Library; stderr was:\n{}",
            stderr(&output)
        );
    }

    assert!(!device.libraries().exists() || device.libraries().read_dir().unwrap().count() == 0);
}

// FM-18: which Library a place holds is read out of the name of its app folder,
// so somewhere that is not one is refused rather than recorded — and refused
// before a Passphrase is chosen.
#[test]
fn joining_somewhere_that_is_not_a_library_is_refused() {
    let device = Device::new();
    let created = init_s3(&device, "joinable");
    let prefix = printed_prefix(&created);

    let elsewhere = device.run(&[
        "join",
        "--name",
        "elsewhere",
        "--recovery-code-stdin",
        "--s3",
        "--bucket",
        "photos",
        // The base the Library was created under rather than the Library's own
        // prefix: it stands for every Library kept at that location.
        "--prefix",
        "archive/",
        "--endpoint",
        stub_endpoint(),
        "--region",
        REGION,
        "--path-style",
        "--passphrase-stdin",
    ]);
    assert_eq!(code(&elsewhere), 1);
    assert!(!device.libraries().join("elsewhere").exists());
    let said = stderr(&elsewhere);
    assert!(said.contains("is not where a Library lives"), "{said}");
    assert!(!said.contains("standard input ended"), "{said}");

    let bad_name = device.run(&[
        "join",
        "--name",
        "not/one/name",
        "--recovery-code-stdin",
        "--s3",
        "--bucket",
        "photos",
        "--prefix",
        &prefix,
        "--endpoint",
        stub_endpoint(),
        "--region",
        REGION,
        "--path-style",
        "--passphrase-stdin",
    ]);
    assert_eq!(code(&bad_name), 1);
    let said = stderr(&bad_name);
    assert!(said.contains("cannot name a Library"), "{said}");
    assert!(!said.contains("standard input ended"), "{said}");

    // And a code that is not one is refused the same way, whatever place it is
    // offered with (spec: KD-11).
    let mistyped = device.run_with(
        &[
            "join",
            "--name",
            "mistyped",
            "--recovery-code-stdin",
            "--s3",
            "--bucket",
            "photos",
            "--prefix",
            &prefix,
            "--endpoint",
            stub_endpoint(),
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some("coffret1not-a-code"),
    );
    assert_eq!(code(&mistyped), 1);
    assert!(!device.libraries().join("mistyped").exists());
}

#[test]
fn join_reads_the_recovery_code_then_the_new_passphrase_from_separate_lines() {
    let device = Device::new();
    let created = init_s3(&device, "source");
    let recovery_code = printed_code(&created);
    let prefix = printed_prefix(&created);
    let joined_passphrase = "a passphrase belonging to the joined device";
    let input = format!("{recovery_code}\n{joined_passphrase}");

    let joined = device.run_with(
        &[
            "join",
            "--name",
            "joined",
            "--recovery-code-stdin",
            "--s3",
            "--bucket",
            "photos",
            "--prefix",
            &prefix,
            "--endpoint",
            stub_endpoint(),
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(&input),
    );

    succeeded(&joined, "join");
    assert!(device.libraries().join("joined").exists());
    assert!(!stdout(&joined).contains(&recovery_code));
    assert!(!stderr(&joined).contains(&recovery_code));

    let reopened = device.run_with(
        &["recovery-code", "--library", "joined", "--passphrase-stdin"],
        Some(joined_passphrase),
    );
    succeeded(&reopened, "recovery-code on the joined device");
    assert_eq!(printed_code(&reopened), recovery_code);
}

#[test]
fn join_reports_missing_invalid_and_overlong_input_without_leaking_it() {
    let device = Device::new();
    let created = init_s3(&device, "input-errors");
    let prefix = printed_prefix(&created);
    let arguments = [
        "join",
        "--name",
        "not-joined",
        "--recovery-code-stdin",
        "--s3",
        "--bucket",
        "photos",
        "--prefix",
        &prefix,
        "--endpoint",
        stub_endpoint(),
        "--region",
        REGION,
        "--path-style",
        "--passphrase-stdin",
    ];

    let missing = device.run(&arguments);
    assert_eq!(code(&missing), 1);
    assert!(
        stderr(&missing).contains("standard input ended before a Recovery Code was given"),
        "{}",
        stderr(&missing)
    );

    for (input, refusal, secret_fragment) in [
        (String::new(), "an empty line is not a Recovery Code", None),
        (
            "private-invalid-recovery-code".to_owned(),
            "what was entered is not a Recovery Code",
            Some("private-invalid-recovery-code"),
        ),
        (
            "private-overlong-recovery-code".repeat(12),
            "the Recovery Code line is longer than 256 bytes",
            Some("private-overlong-recovery-code"),
        ),
    ] {
        let refused = device.run_with(&arguments, Some(&input));
        assert_eq!(code(&refused), 1);
        let out = stdout(&refused);
        let said = stderr(&refused);
        assert!(said.contains(refusal), "{said}");
        if let Some(secret_fragment) = secret_fragment {
            assert!(!out.contains(secret_fragment), "{out}");
            assert!(!said.contains(secret_fragment), "{said}");
        }
        assert!(!device.libraries().join("not-joined").exists());
    }
}

// DK-10: what `join` offers is the prompt or standard input, and no option
// that would take either secret as an argument — the spelling that once did is
// refused, and the refusal does not repeat the value it was handed. The help
// also states the order the two lines come in, since that is the only place a
// script writer looks for it.
#[test]
fn join_help_names_only_secret_input_and_states_the_two_line_order() {
    let device = Device::new();
    let help = device.run(&["join", "--help"]);
    succeeded(&help, "join --help");
    let help = stdout(&help)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(help.contains("--recovery-code-stdin"), "{help}");
    assert!(!help.contains("--recovery-code <"), "{help}");
    assert!(
        help.contains("Recovery Code on the first line and this device's Passphrase on the next"),
        "{help}"
    );

    let rejected = device.run(&["join", "--recovery-code", "old-argv-value"]);
    assert_eq!(code(&rejected), 1);
    assert!(!stderr(&rejected).contains("old-argv-value"));
}

// DK-10 for the scripts this repository runs itself: they hand both secrets
// over on the pipe, in the two-line order, and put neither in argv — a code in
// a command line would be a code in a process listing and in whatever
// transcript the run leaves behind.
#[test]
fn active_round_trip_scripts_keep_the_recovery_code_out_of_join_arguments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    for relative in ["scripts/e2e-it.sh", "scripts/drive-round-trip-it.sh"] {
        let script =
            fs::read_to_string(root.join(relative)).expect("the active script is readable");
        assert!(script.contains("--recovery-code-stdin"), "{relative}");
        let code_piped = script
            .find("printf '%s\\n' \"$recovery_code\"")
            .unwrap_or_else(|| panic!("{relative} must feed the code through the pipe"));
        let passphrase_piped = script
            .find("printf '%s\\n' \"$PASSPHRASE\"")
            .unwrap_or_else(|| panic!("{relative} must feed the Passphrase through the pipe"));
        assert!(
            code_piped < passphrase_piped,
            "{relative} must feed the code before the Passphrase"
        );
        // Every way a shell hands an option a value, rather than the one
        // spelling these scripts used to carry: a guard tied to one variable
        // name would pass a rewrite that put the code back in argv under
        // another one. Neither form appears in `--recovery-code-stdin`, where
        // what follows the option name is `-stdin`.
        for argv_form in ["--recovery-code ", "--recovery-code="] {
            assert!(
                !script.contains(argv_form),
                "{relative} must not put the code in argv or a command transcript"
            );
        }
    }
}

// On S3 nothing about setting a Library up would notice a bucket that is not
// there, so `init` asks — and a person hears it now rather than at the first
// sync, with a Recovery Code already written down for a Library that is nowhere.
#[test]
fn a_bucket_that_does_not_answer_creates_nothing() {
    let device = Device::new();

    // A port nothing is listening at, which is what a mistyped endpoint and an
    // implementation that is not running both look like.
    let output = device.run_with(
        &[
            "init",
            "--name",
            "nowhere",
            "--s3",
            "--bucket",
            "absent-bucket",
            "--endpoint",
            "http://127.0.0.1:1",
            "--region",
            REGION,
            "--path-style",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );

    assert_eq!(code(&output), 1);
    assert!(
        stderr(&output).contains("absent-bucket"),
        "the refusal must name the bucket: {:?}",
        stderr(&output)
    );
    assert!(!stdout(&output).contains(RECOVERY_CODE_PREFIX));
    assert!(!device.libraries().join("nowhere").exists());
}

// EP-2: `--under` is read before the Passphrase is asked for. A path with a
// trailing separator is the caller's own typo, and nobody should have to type a
// secret to be told about one — so the run refuses with an empty standard input
// under it and never mentions the Passphrase.
#[test]
fn a_prefix_that_is_not_an_entry_path_is_refused_before_the_passphrase_is_asked() {
    let device = Device::new();
    init_s3(&device, "typo");

    let refused = device.run_with(
        &[
            "fetch",
            "--library",
            "typo",
            "--under",
            "albums/",
            "--passphrase-stdin",
        ],
        None,
    );

    assert_ne!(
        code(&refused),
        0,
        "a path that is not one is a refusal; stderr was:\n{}",
        stderr(&refused)
    );
    let said = stderr(&refused);
    assert!(
        said.contains("ends with a separator"),
        "the refusal names the part of the shape that went:\n{said}"
    );
    assert!(
        !said.to_lowercase().contains("passphrase"),
        "nothing was asked of the Passphrase before the path was read:\n{said}"
    );
}
