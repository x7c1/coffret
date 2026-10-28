//! What the command line answers a script with under `--json`.
//!
//! The object each command prints is a contract: the hardware targets and
//! `e2e-it` read their facts out of it rather than out of sentences written for
//! a person. So each case here pins the whole object a command answered with —
//! a golden answer — with the values that differ from run to run (the log file,
//! the Library ID, the Recovery Code) checked for their form and then put in
//! the golden as placeholders.
//!
//! The Libraries here are S3 ones against `support::stub_endpoint`, as in
//! `setup.rs`, which is a bucket in memory that holds what the cases write and
//! nothing else: enough for every command to run to its answer without Docker.
//! What a run that carried files answers is pinned in the answer module's own
//! cases, over outcomes assembled by hand, and exercised end to end by
//! `e2e-it`.

mod support;

use std::process::Output;

use serde_json::{json, Value};

use support::{
    code, stderr, stdout, stub_endpoint, succeeded, Device, PASSPHRASE, RECOVERY_CODE_PREFIX,
    REGION,
};

/// The placeholder a value that differs from run to run is given in a golden.
const VARIES: &str = "<varies>";

/// The one JSON object a run printed, having asserted it is the whole of
/// standard output: one line, and nothing before or after it.
fn answer(output: &Output) -> Value {
    let printed = stdout(output);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines.len(),
        1,
        "--json prints one line and nothing else on standard output; it printed:\n{printed}\n\
         stderr was:\n{}",
        stderr(output),
    );
    let value: Value = serde_json::from_str(lines[0])
        .unwrap_or_else(|error| panic!("standard output is not JSON ({error}): {printed}"));
    assert!(value.is_object(), "the answer is one object: {value}");
    value
}

/// `answer` with the log file checked against the line the run printed on
/// standard error, and replaced by [`VARIES`].
fn answer_logged(output: &Output) -> Value {
    let mut value = answer(output);
    let log = value["log"]
        .as_str()
        .unwrap_or_else(|| panic!("a run that started names its log file: {value}"))
        .to_owned();
    assert!(
        stderr(output).contains(&format!("Logging this run to {log}.")),
        "the log is the one the run said it was writing to: {value}\n{}",
        stderr(output),
    );
    value["log"] = json!(VARIES);
    value
}

/// Replaces the string at `pointer` with [`VARIES`], handing back what was
/// there.
fn varies(value: &mut Value, pointer: &str) -> String {
    let slot = value
        .pointer_mut(pointer)
        .unwrap_or_else(|| panic!("the answer holds {pointer}"));
    let was = slot
        .as_str()
        .unwrap_or_else(|| panic!("{pointer} is a string: {slot}"))
        .to_owned();
    *slot = json!(VARIES);
    was
}

/// Creates an S3 Library called `name` on `device`, answering in JSON.
fn init(device: &Device, name: &str) -> Output {
    let output = device.run_with(
        &[
            "--json",
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

/// A Library called `name` on `device`, and what `init` answered for it, with
/// its Library ID and Recovery Code handed back.
fn created(device: &Device, name: &str) -> (Value, String, String) {
    let mut value = answer_logged(&init(device, name));
    let library_id = varies(&mut value, "/answer/library_id");
    let recovery_code = varies(&mut value, "/answer/recovery_code");
    let prefix = varies(&mut value, "/answer/storage/prefix");
    assert_eq!(prefix, format!("archive/coffret-{library_id}/"));
    assert!(
        recovery_code.starts_with(RECOVERY_CODE_PREFIX),
        "{recovery_code}"
    );
    (value, library_id, recovery_code)
}

#[test]
fn init_answers_where_the_library_is_and_its_recovery_code() {
    let device = Device::new();
    let (value, _, _) = created(&device, "alpha");
    assert_eq!(
        value,
        json!({
            "version": 1,
            "command": "init",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                "library": "alpha",
                "library_id": VARIES,
                "storage": { "provider": "s3", "bucket": "photos", "prefix": VARIES },
                "account": null,
                "consent_asked": false,
                "recovery_code": VARIES,
            },
            "error": null,
            "findings": [],
        }),
    );
}

// The code in the answer is the one the text form prints, and it is the one
// that opens the Library: a second device joins with it below.
#[test]
fn join_answers_what_it_found_and_carries_no_recovery_code() {
    let device = Device::new();
    let (_, library_id, recovery_code) = created(&device, "source");
    let prefix = format!("archive/coffret-{library_id}/");
    // A second device: a Library is on a device once.
    let device = Device::new();

    let output = device.run_with(
        &[
            "join",
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
            // After the subcommand, where a script appending it puts it.
            "--json",
        ],
        Some(&format!("{recovery_code}\n{PASSPHRASE}")),
    );
    succeeded(&output, "join");
    assert!(!stdout(&output).contains(&recovery_code));

    let mut value = answer_logged(&output);
    assert_eq!(varies(&mut value, "/answer/library_id"), library_id);
    assert_eq!(
        value,
        json!({
            "version": 1,
            "command": "join",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                // The name it was created with, read from the Library
                // (spec: FM-15).
                "library": "source",
                "library_id": VARIES,
                "storage": { "provider": "s3", "bucket": "photos", "prefix": prefix },
                "account": null,
                "consent_asked": false,
                "found_on_storage": "the_library",
            },
            "error": null,
            "findings": [],
        }),
    );
}

#[test]
fn recovery_code_answers_the_code_init_answered() {
    let device = Device::new();
    let (_, library_id, recovery_code) = created(&device, "again");

    let output = device.run_with(
        &[
            "--json",
            "recovery-code",
            "--library",
            "again",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&output, "recovery-code");
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "recovery-code",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                "library": "again",
                "library_id": library_id,
                "recovery_code": recovery_code,
            },
            "error": null,
            "findings": [],
        }),
    );
}

// The Library and nothing else: neither Passphrase is ever in an answer.
#[test]
fn change_passphrase_answers_the_library() {
    let device = Device::new();
    let (_, library_id, _) = created(&device, "rekeyed");

    let output = device.run_with(
        &[
            "--json",
            "change-passphrase",
            "--library",
            "rekeyed",
            "--passphrase-stdin",
        ],
        Some(&format!("{PASSPHRASE}\na passphrase chosen later")),
    );
    succeeded(&output, "change-passphrase");
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "change-passphrase",
            "exit_status": 0,
            "log": VARIES,
            "answer": { "library": "rekeyed", "library_id": library_id },
            "error": null,
            "findings": [],
        }),
    );
}

// FM-15: a Library this device knows no name for yet — one joined before its
// catalog caught up — is answered with a null name beside its ID, and the text
// says it has none, rather than putting its ID where the name goes.
#[test]
fn a_library_with_no_name_yet_is_answered_with_a_null_name_and_its_id() {
    let device = Device::new();
    let (_, library_id, recovery_code) = created(&device, "nameless");
    forget_the_name(&device, "nameless");

    let output = device.run_with(
        &[
            "--json",
            "recovery-code",
            "--library-id",
            &library_id,
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&output, "recovery-code");
    assert_eq!(
        answer_logged(&output)["answer"],
        json!({
            "library": null,
            "library_id": library_id,
            "recovery_code": recovery_code,
        }),
    );

    for form in [&["--json"][..], &[][..]] {
        let mut args = form.to_vec();
        args.extend([
            "change-passphrase",
            "--library-id",
            &library_id,
            "--passphrase-stdin",
        ]);
        let output = device.run_with(&args, Some(&format!("{PASSPHRASE}\n{PASSPHRASE}")));
        succeeded(&output, "change-passphrase");
        let said = String::from_utf8_lossy(&output.stderr);
        assert!(
            said.contains(&format!(
                "The Passphrase of the Library with ID {library_id} (it has no name yet)"
            )),
            "{said}"
        );
        if !form.is_empty() {
            assert_eq!(
                answer_logged(&output)["answer"],
                json!({ "library": null, "library_id": library_id }),
            );
        }
    }
}

/// Takes the copy of the Library `name`'s name out of the settings beside it,
/// as a Library joined before its catalog caught up has none.
fn forget_the_name(device: &Device, name: &str) {
    let settings = device.library(name).join("settings.json");
    let mut value: Value =
        serde_json::from_slice(&std::fs::read(&settings).expect("the settings are readable"))
            .expect("the settings are JSON");
    value
        .as_object_mut()
        .expect("the settings are an object")
        .remove("name")
        .expect("a created Library's settings carry its name");
    std::fs::write(&settings, serde_json::to_vec(&value).unwrap()).expect("written back");
}

#[test]
fn map_and_mappings_answer_the_prefix_and_the_folder() {
    let device = Device::new();
    created(&device, "listed");
    let albums = device.folder("albums");
    let albums = albums.canonicalize().expect("the folder exists");
    let albums = albums.to_str().expect("the folder has a usable name");

    let mapped = device.run(&[
        "--json",
        "map",
        "--library",
        "listed",
        "--prefix",
        "albums",
        albums,
    ]);
    succeeded(&mapped, "map");
    assert_eq!(
        answer_logged(&mapped),
        json!({
            "version": 1,
            "command": "map",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                "prefix": "albums",
                "local_root": albums,
                "replaced": null,
                "marker": "written",
            },
            "error": null,
            "findings": [],
        }),
    );

    let listed = device.run(&["--json", "mappings", "--library", "listed"]);
    succeeded(&listed, "mappings");
    assert_eq!(
        answer_logged(&listed),
        json!({
            "version": 1,
            "command": "mappings",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                "mappings": [{ "prefix": "albums", "local_root": albums }],
                "refused": null,
            },
            "error": null,
            "findings": [],
        }),
    );
}

/// What a command acting on the Library answers on a device that maps nothing,
/// with a Library created for it.
fn on_a_device_mapping_nothing(arguments: &[&str]) -> Output {
    let device = Device::new();
    created(&device, "empty");
    device.run_with(arguments, Some(PASSPHRASE))
}

// The counts, and the one state they cannot say: `mappings` is 0, which the
// text form says in a sentence of its own.
#[test]
fn sync_answers_its_counts_and_that_it_committed_nothing() {
    let output = on_a_device_mapping_nothing(&[
        "--json",
        "sync",
        "--library",
        "empty",
        "--passphrase-stdin",
    ]);
    succeeded(&output, "sync");
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "sync",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                "added": 0,
                "replaced": 0,
                "renamed": 0,
                "unchanged": 0,
                "committed_head": null,
                "mappings": 0,
            },
            "error": null,
            "findings": [],
        }),
    );
}

// A sync reports progress as it goes and says a sentence about a device that
// maps nothing, and under `--json` both are on standard error: standard output
// is the one object, which `answer` asserts is the whole of it.
#[test]
fn sync_under_json_puts_its_progress_and_advice_on_standard_error() {
    let output = on_a_device_mapping_nothing(&[
        "--json",
        "sync",
        "--library",
        "empty",
        "--passphrase-stdin",
    ]);
    succeeded(&output, "sync");
    answer(&output);
    let said = stderr(&output);
    assert!(said.contains("maps no folder"), "{said}");
    assert!(
        said.lines()
            .any(|line| line == "catching up with the Library"),
        "the run said what it was doing, on standard error: {said}"
    );
}

#[test]
fn freeze_answers_its_counts() {
    let output = on_a_device_mapping_nothing(&[
        "--json",
        "freeze",
        "--library",
        "empty",
        "--passphrase-stdin",
    ]);
    succeeded(&output, "freeze");
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "freeze",
            "exit_status": 0,
            "log": VARIES,
            "answer": {
                "packs": 0,
                "entries": 0,
                "absorbed": 0,
                "packed_already": 0,
                "committed_head": null,
                "mappings": 0,
            },
            "error": null,
            "findings": [],
        }),
    );
}

#[test]
fn fetch_answers_its_counts() {
    let output = on_a_device_mapping_nothing(&[
        "--json",
        "fetch",
        "--library",
        "empty",
        "--passphrase-stdin",
    ]);
    succeeded(&output, "fetch");
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "fetch",
            "exit_status": 0,
            "log": VARIES,
            "answer": { "fetched": 0, "containers": 0, "skipped": 0, "mappings": 0 },
            "error": null,
            "findings": [],
        }),
    );
}

// A mapping whose folder is not there cannot be opened, and is refused with
// the variant's own name and the sentence the text form prints.
#[test]
fn a_mapping_that_cannot_be_opened_answers_its_kind() {
    let device = Device::new();
    created(&device, "missing");
    let gone = device.folder("gone");
    std::fs::remove_dir(&gone).expect("the folder can be removed");
    let gone = gone.to_str().expect("the folder has a usable name");

    let output = device.run(&["--json", "map", "--library", "missing", gone]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "map",
            "exit_status": 1,
            "log": VARIES,
            "answer": null,
            "error": {
                "kind": "no_such_local_root",
                "message": format!(
                    "{gone} is not a directory on this device: No such file or directory \
                     (os error 2)"
                ),
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// A rename of a path the Library holds nothing at is refused by its own kind,
// with nothing committed, and answers no counts.
#[test]
fn a_rename_of_nothing_answers_nothing_to_rename() {
    let output = on_a_device_mapping_nothing(&[
        "--json",
        "rename",
        "--library",
        "empty",
        // Before the paths, which a positional path would have made read as
        // a Passphrase typed after the flag.
        "--passphrase-stdin",
        "--from",
        "albums/a.jpg",
        "--to",
        "albums/b.jpg",
    ]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert_eq!(
        answer_logged(&output),
        json!({
            "version": 1,
            "command": "rename",
            "exit_status": 1,
            "log": VARIES,
            "answer": null,
            "error": {
                "kind": "nothing_to_rename",
                "message": "the rename did not finish: the rename was refused: the Library \
                            holds no Entry at or under \"albums/a.jpg\"",
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// The refusals that need no catalog say so in text too, on standard error,
// and a path that is not an Entry Path is refused before the Passphrase is
// read (spec: EP-2).
#[test]
fn a_rename_refused_in_text_says_why_on_standard_error() {
    let device = Device::new();
    created(&device, "text");

    for (from, to, said) in [
        (
            "albums/a.jpg",
            "albums/a.jpg",
            "the rename was refused: the new Entry Path is the one it already has",
        ),
        (
            "albums/a.jpg",
            ".coffret-derived/a.jpg",
            "lies under .coffret-derived, which coffret keeps for derived Entries",
        ),
    ] {
        let output = device.run_with(
            &[
                "rename",
                "--library",
                "text",
                "--from",
                from,
                "--to",
                to,
                "--passphrase-stdin",
            ],
            Some(PASSPHRASE),
        );
        assert_eq!(code(&output), 1, "{}", stderr(&output));
        assert_eq!(stdout(&output), "", "a refusal answers nothing");
        assert!(stderr(&output).contains(said), "{}", stderr(&output));
    }

    let output = device.run(&[
        "--json",
        "rename",
        "--library",
        "text",
        "--from",
        "albums/",
        "--to",
        "b",
    ]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    assert_eq!(answer_logged(&output)["error"]["kind"], "bad_path");
}

// A Library just created holds its first head and nothing older, so a prune
// writes the head's checkpoint and removes nothing; run again straight away,
// it finds that checkpoint there and still removes nothing. Both succeed.
#[test]
fn prune_answers_the_checkpoint_it_rested_on_and_what_it_removed() {
    let device = Device::new();
    created(&device, "pruned");
    let prune = || {
        device.run_with(
            &[
                "--json",
                "prune",
                "--library",
                "pruned",
                "--passphrase-stdin",
            ],
            Some(PASSPHRASE),
        )
    };

    for wrote_snapshot in [true, false] {
        let output = prune();
        succeeded(&output, "prune");
        assert_eq!(
            answer_logged(&output),
            json!({
                "version": 1,
                "command": "prune",
                "exit_status": 0,
                "log": VARIES,
                "answer": {
                    "head": 0,
                    "snapshot": "idx-0.cfrt",
                    "wrote_snapshot": wrote_snapshot,
                    "removed": 0,
                    "oldest_removed": null,
                    "newest_removed": null,
                    "unremoved": [],
                },
                "error": null,
                "findings": [],
            }),
        );
    }

    let output = device.run_with(
        &["prune", "--library", "pruned", "--passphrase-stdin"],
        Some(PASSPHRASE),
    );
    succeeded(&output, "prune");
    assert_eq!(
        stdout(&output),
        "checkpoint idx-0.cfrt was already there, removed no Journal record, kept head 0\n",
    );
}

// The command is listed, and its help says what it removes, what it never
// removes, and that nothing runs it without being asked.
#[test]
fn prune_help_says_what_it_removes_and_that_it_runs_only_when_asked() {
    let device = Device::new();
    let listed = device.run(&["--help"]);
    succeeded(&listed, "--help");
    assert!(stdout(&listed).contains("prune"), "{}", stdout(&listed));

    let help = device.run(&["prune", "--help"]);
    succeeded(&help, "prune --help");
    let help = stdout(&help)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for said in [
        "Remove the Journal records the checkpoint of the Library's head covers",
        "the head itself stays",
        "No checkpoint, Keyring replica or file's content is ever removed",
        "It runs only when asked",
        "--passphrase-stdin",
    ] {
        assert!(help.contains(said), "{said:?} is not in: {help}");
    }
}

// A Library just created holds no Pack, so a repack of any folder — the whole
// Library named as `/` included — selects nothing and commits nothing. And a
// second run is the same answer: there is nothing out of order to rewrite.
#[test]
fn repack_answers_its_counts() {
    let device = Device::new();
    created(&device, "repacked");
    for under in ["books/series", "/"] {
        let output = device.run_with(
            &[
                "--json",
                "repack",
                "--library",
                "repacked",
                "--under",
                under,
                "--passphrase-stdin",
            ],
            Some(PASSPHRASE),
        );
        succeeded(&output, "repack");
        assert_eq!(
            answer_logged(&output),
            json!({
                "version": 1,
                "command": "repack",
                "exit_status": 0,
                "log": VARIES,
                "answer": {
                    "selected": 0,
                    "kept": 0,
                    "removed": 0,
                    "added": 0,
                    "uploaded": 0,
                    "committed_head": null,
                },
                "error": null,
                "findings": [],
            }),
        );
    }

    let output = device.run_with(
        &[
            "repack",
            "--library",
            "repacked",
            "--under",
            "books",
            "--passphrase-stdin",
        ],
        Some(PASSPHRASE),
    );
    succeeded(&output, "repack");
    assert_eq!(
        stdout(&output),
        "selected 0 packs, kept 0, removed 0, added 0, uploaded 0 bytes, committed nothing\n",
    );
}

// The folder is never assumed: a repack without `--under` is refused as what
// was typed, before any Passphrase is asked for.
#[test]
fn a_repack_without_a_folder_answers_usage() {
    let device = Device::new();
    let output = device.run(&["--json", "repack", "--library", "anything"]);
    assert_eq!(code(&output), 1, "{}", stderr(&output));
    let answered = answer(&output);
    assert_eq!(answered["command"], Value::Null);
    assert_eq!(answered["error"]["kind"], "usage", "{answered}");
    assert_eq!(
        answered["error"]["message"],
        "the following required arguments were not provided:",
    );
    assert!(stderr(&output).contains("--under"), "{}", stderr(&output));
}

// The command is listed, and its help says what it rewrites, how to name the
// whole Library, that a second run commits nothing, and that nothing runs it
// without being asked.
#[test]
fn repack_help_says_what_it_rewrites_and_that_it_runs_only_when_asked() {
    let device = Device::new();
    let listed = device.run(&["--help"]);
    succeeded(&listed, "--help");
    assert!(stdout(&listed).contains("repack"), "{}", stdout(&listed));

    let help = device.run(&["repack", "--help"]);
    succeeded(&help, "repack --help");
    let help = stdout(&help)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for said in [
        "Rewrite the Packs holding a folder's files into Packs in path order",
        "`/` names the whole Library",
        "running it again straight away commits nothing",
        "If any of them cannot be read, nothing is changed",
        "take about twice their size on this device's disk",
        "It runs only when asked",
        "--target",
        "--passphrase-stdin",
    ] {
        assert!(help.contains(said), "{said:?} is not in: {help}");
    }
}

// What was typed wrongly is refused before any command runs, and a script that
// asked for JSON still gets its one object.
#[test]
fn a_usage_error_under_json_answers_usage() {
    let device = Device::new();
    let output = device.run(&["--json", "sync", "--no-such-flag"]);
    assert_eq!(code(&output), 1);
    assert_eq!(
        answer(&output),
        json!({
            "version": 1,
            "command": null,
            "exit_status": 1,
            "log": null,
            "answer": null,
            "error": {
                "kind": "usage",
                "message": "unexpected argument '--no-such-flag' found",
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// Without the flag nothing changes: the text form of a sync and of a fetch is
// byte for byte what it was before `--json` existed.
#[test]
fn without_json_sync_and_fetch_print_the_text_they_always_printed() {
    let device = Device::new();
    created(&device, "text");

    let synced = device.run_with(
        &["sync", "--library", "text", "--passphrase-stdin"],
        Some(PASSPHRASE),
    );
    succeeded(&synced, "sync");
    assert_eq!(
        stdout(&synced),
        "added 0, replaced 0, unchanged 0, committed nothing\n\
         this device maps no folder, so there is nothing of this device to carry into the \
         Library: record one with `coffret map` and run this again\n",
    );

    let fetched = device.run_with(
        &["fetch", "--library", "text", "--passphrase-stdin"],
        Some(PASSPHRASE),
    );
    succeeded(&fetched, "fetch");
    assert_eq!(
        stdout(&fetched),
        "fetched 0, containers 0, skipped 0\n\
         this device maps no folder, so there is nowhere for the Library to go: record one \
         with `coffret map` and run this again\n",
    );
}
