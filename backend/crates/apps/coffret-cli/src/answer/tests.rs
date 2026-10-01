//! The golden answers: the whole object a run prints under `--json`, for
//! outcomes and failures assembled by hand.
//!
//! What the built binary answers is pinned in `tests/json.rs` for every
//! command, against a bucket that holds nothing. These are the answers that
//! need a Library with something in it — counts that are not zero, findings,
//! and the refusals a script branches on — which a unit case can assemble and
//! an ordinary test run cannot reach on real Storage.

use std::path::Path;

use serde_json::{json, Value};

use coffret_device::{
    CommitError, ContainerId, EntryFetch, EntryPath, Error, FetchError, FetchOutcome, Finding,
    FindingReason, Findings, IndexError, ProviderSettings, StorageError, SyncError, SyncOutcome,
};

use super::*;
use crate::refusal::Refusal;

/// The log file every answer here names.
const LOG: &str = "/state/logs/coffret-20260101T000000Z-000.log";

/// `document` as the JSON it prints.
fn printed(document: &Document<'_>) -> Value {
    serde_json::from_str(&document.rendered()).expect("the answer is JSON")
}

/// What a run of `command` that succeeded with `ran` prints.
fn succeeded(command: &'static str, ran: &Ran) -> Value {
    printed(&Document::succeeded(command, Some(Path::new(LOG)), ran))
}

/// What a run of `command` that failed with `error` prints.
fn failed(command: &'static str, error: impl Into<anyhow::Error>) -> Value {
    let error = error.into();
    printed(&Document::failed(
        Some(command),
        Some(Path::new(LOG)),
        Failure::of(&error),
        &Findings::repaired_before(error.as_ref()),
    ))
}

fn path(literal: &str) -> EntryPath {
    EntryPath::parse(literal).expect("the literal is an Entry Path")
}

fn container(seed: u8) -> ContainerId {
    ContainerId::from_bytes([seed; ContainerId::BYTE_LEN])
}

/// A file this device had and is gone from disk, as a sync surfaces it.
fn deleted(literal: &str) -> Finding {
    Finding::Surfaced {
        path: path(literal),
        reason: FindingReason::DeletedLocally,
    }
}

// A sync that carried files and surfaced one: the counts, the head it did not
// commit, and the finding by its path and reason beside the line the text form
// prints for it — and the exit status that finding turns.
#[test]
fn a_sync_answers_its_counts_and_its_findings() {
    let outcome = SyncOutcome {
        added: vec![container(1), container(2)],
        replaced: vec![container(3)],
        unchanged: 4,
        mappings: 1,
        surfaced: Vec::new(),
        unavailable: Vec::new(),
        settled: Vec::new(),
        commit: None,
    };
    let findings = Findings::assembled([deleted("albums/a.jpg")]);
    let ran = Ran::found(
        crate::report::findings(&findings, Form::Json),
        Answer::Synced(Synced::from(&outcome)),
        &findings,
    );

    assert_eq!(
        succeeded("sync", &ran),
        json!({
            "version": 1,
            "command": "sync",
            "exit_status": 2,
            "log": LOG,
            "answer": {
                "added": 2,
                "replaced": 1,
                "unchanged": 4,
                "committed_head": null,
                "mappings": 1,
            },
            "error": null,
            "findings": [{
                "kind": "surfaced",
                "needs_attention": true,
                "said": "surfaced albums/a.jpg: this device had it and it is gone from disk",
                "path": "albums/a.jpg",
                "reason": "DeletedLocally",
            }],
        }),
    );
}

#[test]
fn a_fetch_answers_its_counts() {
    let outcome = FetchOutcome {
        fetched: vec![path("albums/a.jpg"), path("albums/b.jpg")],
        containers: vec![container(1)],
        skipped: 3,
        mappings: 2,
        surfaced: Vec::new(),
        refused: Vec::new(),
        locked: Vec::new(),
        degraded: None,
    };
    let ran = Ran::clean(Answer::Fetched(Fetched::from(&outcome)));

    assert_eq!(
        succeeded("fetch", &ran),
        json!({
            "version": 1,
            "command": "fetch",
            "exit_status": 0,
            "log": LOG,
            "answer": { "fetched": 2, "containers": 1, "skipped": 3, "mappings": 2 },
            "error": null,
            "findings": [],
        }),
    );
}

// The one-Entry form's three answers, and the counts the text form prints for
// each of them.
#[test]
fn a_fetch_of_one_entry_answers_which_of_its_three_answers_it_is() {
    let answered = |fetched: &EntryFetch| {
        serde_json::to_value(FetchedEntry::from(fetched)).expect("the answer is JSON")
    };
    assert_eq!(
        answered(&EntryFetch::Placed),
        json!({ "entry": "placed", "fetched": 1, "skipped": 0 }),
    );
    assert_eq!(
        answered(&EntryFetch::AlreadyPresent),
        json!({ "entry": "already_present", "fetched": 0, "skipped": 1 }),
    );
}

// The refusal the dead-grant checks branch on: a Drive Library with no grant,
// its kind the device error's variant, and the sentence that names the command
// to run.
#[test]
fn a_library_with_no_grant_answers_not_authorized() {
    let error = Error::NotAuthorized {
        name: "main".to_owned(),
        cause: None,
    };

    assert_eq!(
        failed("sync", error),
        json!({
            "version": 1,
            "command": "sync",
            "exit_status": 1,
            "log": LOG,
            "answer": null,
            "error": {
                "kind": "not_authorized",
                "message": "the Library \"main\" has no usable grant on Google Drive; run \
                            `coffret authorize --library main`",
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// The other way a grant dies: a refresh token the provider will not take, which
// arrives inside a flow as Storage having rejected the credentials.
#[test]
fn credentials_storage_rejected_answer_unauthenticated_whichever_flow_met_them() {
    let error = Error::from(SyncError::Storage(StorageError::Unauthenticated {
        detail: "the token has expired".to_owned(),
        source: None,
    }));

    let answered = failed("sync", error);
    assert_eq!(answered["error"]["kind"], "unauthenticated", "{answered}");
}

// Storage not coming through is `storage` under a flow, as the explorer's
// server names it, rather than the flow's own name.
#[test]
fn storage_not_answering_under_a_flow_answers_storage() {
    let error = Error::from(SyncError::Storage(StorageError::Timeout {
        detail: "no answer".to_owned(),
        source: None,
    }));

    let answered = failed("sync", error);
    assert_eq!(answered["error"]["kind"], "storage", "{answered}");
}

// An Index file older than this build carries forward, refused inside a sync:
// the kind, and the two layouts as numbers rather than inside the sentence.
#[test]
fn an_index_at_an_older_layout_answers_unsupported_schema_with_both_versions() {
    let error = Error::from(SyncError::Index(IndexError::UnsupportedSchema {
        found: 3,
        supported: 5,
    }));
    let message = format!(
        "{:#}",
        anyhow::Error::from(Error::from(SyncError::Index(
            IndexError::UnsupportedSchema {
                found: 3,
                supported: 5,
            },
        )))
    );

    assert_eq!(
        failed("sync", error),
        json!({
            "version": 1,
            "command": "sync",
            "exit_status": 1,
            "log": LOG,
            "answer": null,
            "error": {
                "kind": "unsupported_schema",
                "message": message,
                "advice": [],
                "found": 3,
                "supported": 5,
            },
            "findings": [],
        }),
    );
    assert!(message.contains("schema version 3"), "{message}");
}

// A listing that read its mappings out of a refused file answers them, and
// says why the Index was not opened in the same shape a failure has.
#[test]
fn a_listing_of_a_refused_file_answers_the_refusal_beside_the_mappings() {
    let refusal = IndexError::UnsupportedSchema {
        found: 1,
        supported: 5,
    };
    let answered = serde_json::to_value(Mappings::new(&[], Some(Failure::of_index(&refusal))))
        .expect("the answer is JSON");

    assert_eq!(
        answered,
        json!({
            "mappings": [],
            "refused": {
                "kind": "unsupported_schema",
                "message": refusal.to_string(),
                "advice": [],
                "found": 1,
                "supported": 5,
            },
        }),
    );
}

// A one-Entry fetch no mapping reaches: `unmapped`, the reason the explorer's
// server gives the same refusal, with the sentence and the next step exactly as
// the text form prints them.
#[test]
fn an_entry_no_mapping_reaches_answers_unmapped() {
    let error = crate::fetch::next_step(Error::Fetch {
        cause: Box::new(FetchError::UnmappedEntryPath {
            path: path("albums/a.jpg"),
        }),
    });
    let said = crate::report::failed(&error).said;

    let answered = failed("fetch", error);
    assert_eq!(answered["error"]["kind"], "unmapped", "{answered}");
    assert_eq!(answered["error"]["message"], said[0], "{answered}");
    assert!(
        said[0].ends_with("run this again"),
        "the next step is part of the sentence: {said:?}"
    );
}

// An epoch this device holds no key for, whichever flow met it, by the kind the
// explorer's server gives it.
#[test]
fn an_epoch_answers_epoch() {
    let error = Error::from(SyncError::Commit(coffret_device::CommitFailure {
        error: Box::new(CommitError::EpochActivated {
            generation: coffret_device::Generation::FIRST,
        }),
        repairs: Vec::new(),
    }));

    let answered = failed("sync", error);
    assert_eq!(answered["error"]["kind"], "epoch", "{answered}");
}

// Flags that leave a provider short, which the shell refuses itself once the
// parser has let them through: a kind of their own rather than `other`, with
// the command and the log named, since the command had started.
#[test]
fn flags_that_leave_a_provider_short_answer_flags_missing() {
    let error = Refusal::FlagsMissing {
        provider: "--s3",
        needs: "--bucket and --prefix",
    };

    assert_eq!(
        failed("join", error),
        json!({
            "version": 1,
            "command": "join",
            "exit_status": 1,
            "log": LOG,
            "answer": null,
            "error": {
                "kind": "flags_missing",
                "message": "--s3 needs --bucket and --prefix",
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// A client secret variable set to an empty value: refused by name, in the
// sentence that says what to do about it.
#[test]
fn an_empty_client_secret_answers_empty_client_secret() {
    let error = Refusal::EmptyClientSecret;

    assert_eq!(
        failed("init", error),
        json!({
            "version": 1,
            "command": "init",
            "exit_status": 1,
            "log": LOG,
            "answer": null,
            "error": {
                "kind": "empty_client_secret",
                "message": "COFFRET_DRIVE_CLIENT_SECRET is set to an empty value; set it to \
                            the secret the client was registered with, or unset it where the \
                            client was registered without one",
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// And one that is not Unicode, which is told apart from an empty one: the two
// are put right differently.
#[test]
fn a_client_secret_that_is_not_unicode_answers_client_secret_not_unicode() {
    let error = Refusal::ClientSecretNotUnicode;

    assert_eq!(
        failed("join", error),
        json!({
            "version": 1,
            "command": "join",
            "exit_status": 1,
            "log": LOG,
            "answer": null,
            "error": {
                "kind": "client_secret_not_unicode",
                "message": "COFFRET_DRIVE_CLIENT_SECRET is not valid Unicode",
                "advice": [],
            },
            "findings": [],
        }),
    );
}

// No secret beyond what the text form prints: an S3 Library's endpoint and a
// Drive Library's client are nowhere in where the Library is said to be.
#[test]
fn where_a_library_is_says_nothing_about_how_to_reach_it() {
    let drive = ProviderSettings::Drive {
        folder_id: "1a2B3c".to_owned(),
        client_id: "client.apps.googleusercontent.com".to_owned(),
        client_secret: Some("a secret".to_owned()),
        account: Some("default".to_owned()),
    };
    assert_eq!(
        serde_json::to_value(Storage::from(&drive)).expect("the answer is JSON"),
        json!({ "provider": "drive", "folder_id": "1a2B3c" }),
    );
}

// The version a script checks, and the help text that documents the shape,
// cannot drift apart.
#[test]
fn the_help_names_the_version_it_documents() {
    assert!(
        SHAPE.contains(&format!("version      {VERSION},")),
        "the --help text documents version {VERSION}: {SHAPE}"
    );
}
