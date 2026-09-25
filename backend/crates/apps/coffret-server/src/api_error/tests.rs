use std::fs;
use std::path::{Path, PathBuf};

use coffret_device::{
    CommitError, EntryPath, Error, FetchError, RefusedRoot, RootRefused, Surfaced,
};
use coffret_model::{ContainerId, ContentHash, Generation};
use coffret_usecase::freeze::FreezeError;
use coffret_usecase::root_marker::MalformedMarker;
use coffret_usecase::sync::SyncError;

use super::{name_of, ApiError};
use crate::entry_paths::entry_path;

/// The path every case here refuses something about.
fn path() -> EntryPath {
    entry_path("albums/spring.jpg")
}

fn container_id() -> ContainerId {
    ContainerId::from_bytes([0x11; ContainerId::BYTE_LEN])
}

/// The status, the kind, and the two details, as the wire would carry them.
fn wire(
    error: ApiError,
) -> (
    u16,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
) {
    (
        error.status.as_u16(),
        error.kind,
        error.reason,
        error.surfaced,
    )
}

/// What one fetch failure comes back as.
fn from(
    cause: FetchError,
) -> (
    u16,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
) {
    wire(ApiError::from(Error::Fetch { cause }))
}

// EP-5: the Library holds at most one current Entry at a path, and holding
// none there is the request's answer rather than a failure of anything.
#[test]
fn a_path_the_library_holds_nothing_at_is_not_found() {
    assert_eq!(
        from(FetchError::EntryNotCurrent { path: path() }),
        (404, "no_such_entry", None, None),
    );
}

// EP-9: a mapping is what makes a local path exist, so an Entry outside
// every one of them is a fact about this device — which is what the explorer
// shows as "this folder is not on this device".
#[test]
fn an_entry_no_mapping_reaches_is_declined_as_unmapped() {
    assert_eq!(
        from(FetchError::UnmappedEntryPath { path: path() }),
        (409, "declined", Some("unmapped"), None),
    );
}

// The EP-9 translation asked on its own carries the fetch's vocabulary without
// being a fetch, and a browser is owed the same answer either way: what it can
// do about an unmapped path does not depend on whether a transfer was going to
// follow it.
#[test]
fn a_path_that_could_not_be_placed_is_answered_as_the_fetch_would_answer_it() {
    assert_eq!(
        wire(ApiError::from(Error::LocalPathNotSettled {
            cause: FetchError::UnmappedEntryPath { path: path() },
        })),
        from(FetchError::UnmappedEntryPath { path: path() }),
    );
    assert_eq!(
        wire(ApiError::from(Error::LocalPathNotSettled {
            cause: FetchError::EntryNotCurrent { path: path() },
        })),
        (404, "no_such_entry", None, None),
    );
}

// And a file turned away on its way into a mapped folder, which is the same
// verdicts reported by the write side. Pinned because the arm that carries them
// is one of several onto the same answer and the match has a catch-all under
// it: dropping the arm would compile, and every refused drop would reach the
// browser as a `500` saying nothing about a mapping.
#[test]
fn a_file_that_was_not_taken_in_is_answered_as_the_fetch_would_answer_it() {
    assert_eq!(
        wire(ApiError::from(Error::FileNotTakenIn {
            cause: FetchError::UnmappedEntryPath { path: path() },
        })),
        from(FetchError::UnmappedEntryPath { path: path() }),
    );
    assert_eq!(
        wire(ApiError::from(Error::FileNotTakenIn {
            cause: FetchError::ReservedComponent {
                path: path(),
                component: ".coffret".to_owned(),
            },
        })),
        from(FetchError::ReservedComponent {
            path: path(),
            component: ".coffret".to_owned(),
        }),
    );
}

// And a read of what somebody has put in a mapped folder, which is the third
// caller raising the fetch's vocabulary without fetching. Pinned for the reason
// the one above it is: the arm is one of several onto the same answer with a
// catch-all under it, so dropping it would compile and a folder whose name this
// device cannot tell apart from its own would reach the browser as a `500`
// saying nothing about that folder.
#[test]
fn a_folder_that_could_not_be_read_is_answered_as_the_fetch_would_answer_it() {
    assert_eq!(
        wire(ApiError::from(Error::LocalFilesNotRead {
            cause: FetchError::FoldedReservedComponent {
                path: path(),
                component: ".COFFRET".to_owned(),
            },
        })),
        from(FetchError::FoldedReservedComponent {
            path: path(),
            component: ".COFFRET".to_owned(),
        }),
    );
    assert_eq!(
        wire(ApiError::from(Error::LocalFilesNotRead {
            cause: FetchError::UnmaterializablePath {
                path: path(),
                stopped_at: None,
            },
        })),
        from(FetchError::UnmaterializablePath {
            path: path(),
            stopped_at: None,
        }),
    );
}

// And the opening of the file this device placed for an Entry, which is the
// fourth such caller and the one a browser walks over whenever somebody opens a
// picture. Pinned for the reason the two above it are, and with more riding on
// it: the arm sits over the same catch-all, so dropping it would compile and
// every one of those readings would reach the browser as a `500`.
//
// The second half is the case the file route branches on rather than reports
// (spec: EP-10), and it is here because the route's reading of it is only as
// good as the answer it would otherwise fall through to.
#[test]
fn a_placed_file_that_did_not_open_is_answered_as_the_fetch_would_answer_it() {
    assert_eq!(
        wire(ApiError::from(Error::LocalFileNotOpened {
            cause: FetchError::UnmappedEntryPath { path: path() },
        })),
        from(FetchError::UnmappedEntryPath { path: path() }),
    );
    assert_eq!(
        wire(ApiError::from(Error::LocalFileNotOpened {
            cause: FetchError::EntryNotCurrent { path: path() },
        })),
        (404, "no_such_entry", None, None),
    );
}

// EP-4: a path a mapping does reach and this device still cannot hold a file
// at — two Entry Paths that would land on one local path, or one no filesystem
// here can spell — is refused explicitly rather than by quietly choosing a
// name. The folder is here either way, so it is a different verdict from an
// unmapped path and travels under its own reason.
//
// A blocked descent is the same verdict with the folder it stopped at beside
// it, and the folder changes nothing on the wire: a local path is not something
// a body carries (spec: EL-1).
#[test]
fn a_path_this_device_cannot_hold_a_file_at_is_declined_as_unmaterializable() {
    assert_eq!(
        from(FetchError::UnmaterializablePath {
            path: path(),
            stopped_at: None,
        }),
        (409, "declined", Some("unmaterializable"), None),
    );
    assert_eq!(
        from(FetchError::UnmaterializablePath {
            path: path(),
            stopped_at: Some(PathBuf::from("/home/someone/albums")),
        }),
        (409, "declined", Some("unmaterializable"), None),
    );
    assert_eq!(
        from(FetchError::LocalPathCollision {
            first: path(),
            second: entry_path("albums/SPRING.JPG"),
        }),
        (409, "declined", Some("unmaterializable"), None),
    );
}

// EP-11, EP-14: a path carrying a name coffret keeps for itself inside a mapped
// folder is its own reason. Not `unmaterializable`, which says no local name can
// stand for the path at all: here exactly one name in it is taken, and that is a
// different thing for a browser to say and a different thing to do about.
#[test]
fn a_path_carrying_a_name_coffret_keeps_is_declined_as_reserved() {
    let refusal = ApiError::from(Error::Fetch {
        cause: FetchError::ReservedComponent {
            path: entry_path("albums/.coffret/root"),
            component: ".coffret".to_owned(),
        },
    });
    let message = refusal.message().to_owned();

    assert_eq!(wire(refusal), (409, "declined", Some("reserved"), None));
    // The reserved names are said, both of them: this sentence is read beside
    // the name somebody dropped, and one that named no name at all would leave
    // them guessing which component of their own path it meant (spec: EP-4).
    assert!(
        message.contains("`.coffret`") && message.contains("`.coffret-fetch-`"),
        "the sentence names the names that are taken: {message}",
    );
    assert!(
        !message.contains("albums"),
        "and never the path, which is the person's own name for their file: {message}",
    );
}

// EP-14: a component that only folds to the management area's name is the same
// thing for a browser to branch on — one name in the path is taken — and not the
// same thing to read. Every clause of the sentence above is false about
// `.COFFRET`: it is nothing coffret keeps, no scan steps over it, and two of the
// three callers that raise it are reads rather than placements.
#[test]
fn a_path_carrying_a_folded_spelling_is_declined_as_reserved_and_said_differently() {
    let reserved = ApiError::from(Error::Fetch {
        cause: FetchError::ReservedComponent {
            path: entry_path("albums/.coffret/root"),
            component: ".coffret".to_owned(),
        },
    });
    let refusal = ApiError::from(Error::Fetch {
        cause: FetchError::FoldedReservedComponent {
            path: entry_path("albums/.COFFRET/root"),
            component: ".COFFRET".to_owned(),
        },
    });
    let said = refusal.message().to_owned();

    // The same reason, because adding one is adding a case to every caller and
    // there is nothing here for one of them to do differently.
    assert_eq!(wire(refusal), (409, "declined", Some("reserved"), None));
    assert_ne!(
        said,
        reserved.message(),
        "and never the sentence about a name coffret keeps for itself",
    );
    assert!(
        !said.contains("keeps for itself"),
        "which is what `.COFFRET` is not: {said}",
    );
    // The one name that may be said is coffret's own, and what the sentence
    // offers beside it is how the person's differs — enough to find on a screen
    // without the component, which is a piece of an Entry Path (spec: EL-1).
    assert!(
        said.contains("`.coffret`") && said.contains("case"),
        "the sentence says which name is taken and how theirs differs: {said}",
    );
    assert!(
        !said.contains("albums") && !said.contains(".COFFRET"),
        "and neither the path nor the component: {said}",
    );
}

// EP-13: a mapped folder that is not the folder its mapping was recorded
// against is this device's configuration rather than the server failing, so
// every one of the seven cases reaches the browser as one declined answer with
// a reason of its own — never as the `500` that says only that the server could
// not answer. Shared guidance for all seven names where recovery starts and the
// mapping it is aimed at, since a device has as many as its owner gave it.
#[test]
fn every_refused_root_reaches_the_browser_under_one_declined_reason() {
    for reason in [
        RootRefused::NoExpectedIdentity,
        RootRefused::ManagementAreaMissing,
        RootRefused::ManagementAreaNotADirectory,
        RootRefused::MarkerMissing,
        RootRefused::MarkerNotARegularFile,
        RootRefused::MarkerMalformed {
            cause: MalformedMarker::NotText,
        },
        RootRefused::MarkerMismatch,
    ] {
        // Both mappings a refusal can be about: one standing for a top-level
        // component, and one standing for the Library root, where there is no
        // component to name and the sentence says so instead (spec: EP-9).
        for (prefix, named) in [
            (Some(entry_path("albums")), "\"albums\""),
            (None, "the Library root"),
        ] {
            // Both ways one reaches a route: a fetch that met it while placing,
            // and this device placing the one file an upload handed it.
            let from_fetch = ApiError::from(Error::Fetch {
                cause: FetchError::RefusedRoot(RefusedRoot {
                    prefix: prefix.clone(),
                    local_root: PathBuf::from("/mnt/copied"),
                    reason: reason.clone(),
                }),
            });
            let from_upload = ApiError::from(Error::RootRefused(RefusedRoot {
                prefix: prefix.clone(),
                local_root: PathBuf::from("/mnt/copied"),
                reason: reason.clone(),
            }));

            for refusal in [from_fetch, from_upload] {
                let message = refusal.message().to_owned();
                assert_eq!(
                    wire(refusal),
                    (409, "declined", Some("refused_root"), None),
                    "{reason:?}",
                );
                assert!(
                    message.contains("on the device serving the Library")
                        && message.contains("terminal")
                        && message.contains("coffret mappings --library <library>")
                        && message.contains("coffret map --help")
                        && message.contains("return to the explorer"),
                    "the guidance makes the CLI recovery reachable from the explorer: {message}",
                );
                assert!(
                    message.contains("choose one recovery: reconnect the intended folder")
                        && message.contains(
                            "if the folder at the recorded location is the intended one, record \
                             this mapping again with `coffret map`",
                        )
                        && message.contains(
                            "or map another folder in its place only as a deliberate choice",
                        )
                        && message.contains(
                            "reports a local marker problem, correct the problem and run it again",
                        ),
                    "the guidance keeps reconnection, same-folder recovery, and deliberate \
                     remapping apart: {message}",
                );
                // The mapping is named because the recovery is aimed at one of
                // them, and a prefix is a name inside the Library rather than a
                // path on this device (spec: EL-1).
                assert!(
                    message.contains(named),
                    "the guidance names the mapping it is about: {message}",
                );
                assert!(
                    !message.contains("copied"),
                    "and never the folder, which is a local path: {message}",
                );
            }
        }
    }
}

// EP-11: every Entry a fetch declines says why, and each reason is a
// different thing for a browser to show — so each travels by name.
#[test]
fn each_finding_travels_by_the_name_the_device_layer_gives_it() {
    for (surfaced, reason, name) in [
        (
            Surfaced::ForeignFile { path: path() },
            "surfaced",
            "ForeignFile",
        ),
        (
            Surfaced::LocallyChanged { path: path() },
            "surfaced",
            "LocallyChanged",
        ),
        (
            Surfaced::WitnessedDeletion { path: path() },
            "surfaced",
            "WitnessedDeletion",
        ),
        // EP-4: one folder of the mapped root has a shape no file can be
        // placed through. A finding about this one Entry like the rest, and
        // the folder it names stays out of what goes on the wire.
        (
            Surfaced::UnreachablePlace {
                path: path(),
                stopped_at: PathBuf::from("/home/someone/albums"),
            },
            "surfaced",
            "UnreachablePlace",
        ),
        // KL-7: the one finding nothing about this device can resolve, so
        // it is its own reason rather than one of the others.
        (
            Surfaced::KeyLost {
                path: path(),
                container_id: container_id(),
            },
            "locked",
            "KeyLost",
        ),
        // EP-14: the path carries the name of the device's own folder — or a
        // spelling of it, the two being one finding where what met them is a
        // placement — which is about one Entry and never a place a file is put.
        (
            Surfaced::ReservedComponent {
                path: entry_path("albums/.coffret/root"),
            },
            "surfaced",
            "ReservedComponent",
        ),
    ] {
        assert_eq!(
            wire(ApiError::declined(&surfaced)),
            (409, "declined", Some(reason), Some(name)),
            "{surfaced:?}",
        );
    }
}

// A Storage that did not answer and a Container that did not authenticate
// are both upstream of the browser and both leave nothing on disk
// (spec: EP-11) — so both are 502, told apart only by what the log will say.
#[test]
fn storage_and_a_container_that_does_not_authenticate_are_both_bad_gateways() {
    assert_eq!(
        from(FetchError::Storage(
            coffret_usecase::Error::Unauthenticated {
                detail: "the grant has run out".to_owned(),
                source: None,
            }
        )),
        (502, "storage", None, None),
    );
    assert_eq!(
        from(FetchError::ContainerUnreachable {
            container_id: container_id(),
        }),
        (502, "storage", None, None),
    );
    assert_eq!(
        from(FetchError::Format(
            coffret_format::Error::AuthenticationFailed
        )),
        (502, "unverified", None, None),
    );
    assert_eq!(
        from(FetchError::ContentMismatch {
            container_id: container_id(),
            path: path(),
        }),
        (502, "unverified", None, None),
    );
    assert_eq!(
        from(FetchError::CiphertextMismatch {
            container_id: container_id(),
            expected: ContentHash::from_bytes([0x01; ContentHash::BYTE_LEN]),
            actual: ContentHash::from_bytes([0x02; ContentHash::BYTE_LEN]),
        }),
        (502, "unverified", None, None),
    );
}

// The server's own state, which is nothing the browser did and nothing it
// can do anything about. The body says only that, and the chain goes to the
// log.
#[test]
fn the_servers_own_failures_say_only_that_it_failed() {
    assert_eq!(
        from(FetchError::Index(coffret_usecase::IndexError::NoCheckpoint)),
        (500, "server", None, None),
    );
    assert_eq!(
        wire(ApiError::from(Error::Index {
            cause: coffret_usecase::IndexError::NoCheckpoint,
        })),
        (500, "server", None, None),
    );
}

/// One failure of the commit flow, as each of the four flows that go through it
/// reports it.
///
/// Built afresh for each, because a `CommitError` is not a value that can be
/// copied — and what the cases over this are about is that the flow which met
/// it makes no difference to what it is answered as.
fn from_every_flow(commit: impl Fn() -> CommitError) -> Vec<(&'static str, ApiError)> {
    vec![
        (
            "catch-up",
            ApiError::from(Error::CatchUp { cause: commit() }),
        ),
        (
            "sync",
            ApiError::from(Error::Sync {
                cause: SyncError::Commit(commit()),
            }),
        ),
        (
            "freeze",
            ApiError::from(Error::Freeze {
                cause: FreezeError::Commit(commit()),
            }),
        ),
        (
            "fetch",
            ApiError::from(Error::Fetch {
                cause: FetchError::Commit(commit()),
            }),
        ),
    ]
}

// CP-5, MR-2: an activated epoch is this device's standing in the Library and
// not a failure of anything, so it is its own kind from every flow that can
// meet it — and never the sentence about Storage not answering, which offers a
// retry that can only meet it again. The sentence says what to do and names no
// generation (spec: EL-5).
#[test]
fn an_activated_epoch_is_its_own_kind_from_every_flow() {
    for (flow, refusal) in from_every_flow(|| CommitError::EpochActivated {
        generation: Generation::new(41).expect("a small generation is one"),
    }) {
        let said = refusal.message().to_owned();
        assert_eq!(wire(refusal), (409, "epoch", None, None), "from a {flow}");
        assert!(!said.contains("did not answer"), "from a {flow}: {said}");
        assert!(said.contains("enrolled"), "from a {flow}: {said}");
        assert!(!said.contains("41"), "from a {flow}: {said}");
    }
}

// The commit's own verdicts are classified once, so a sync, a freeze or a fetch
// that met one says what the catch-up says rather than filing it as Storage not
// answering. A slot lost too often is the one here; the rest share its arm. A
// head that would not open is on Storage's side of the line, and is `storage`
// from every flow for the same reason.
#[test]
fn a_commit_verdict_is_answered_alike_from_every_flow() {
    for (flow, refusal) in from_every_flow(|| CommitError::ConflictLimitReached { attempts: 8 }) {
        assert_eq!(
            refusal.message(),
            "the server could not answer",
            "from a {flow}"
        );
        assert_eq!(wire(refusal), (500, "server", None, None), "from a {flow}");
    }
    for (flow, refusal) in from_every_flow(|| CommitError::MissingHead {
        generation: Generation::new(7).expect("a small generation is one"),
    }) {
        assert_eq!(wire(refusal), (502, "storage", None, None), "from a {flow}");
    }
}

// A listing that did not end within the pages this device reads is still on
// Storage's side, and is still `storage` — but Storage answered every page, so
// the sentence says the listing ran past its cap rather than that nothing came
// back. One sentence from both flows that list.
#[test]
fn a_listing_past_its_cap_says_so_from_both_flows_that_list() {
    let refusals = [
        ApiError::from(Error::Sync {
            cause: SyncError::ListingLimitReached { pages: 10_000 },
        }),
        ApiError::from(Error::Freeze {
            cause: FreezeError::ListingLimitReached { pages: 10_000 },
        }),
    ];
    let said: Vec<String> = refusals
        .iter()
        .map(|refusal| refusal.message().to_owned())
        .collect();
    assert_eq!(said[0], said[1], "one sentence from both");
    assert!(said[0].contains("ran past the cap"), "{}", said[0]);
    assert!(!said[0].contains("did not answer"), "{}", said[0]);
    for refusal in refusals {
        assert_eq!(wire(refusal), (502, "storage", None, None));
    }
}

// The same verdict reached through the Storage port rather than raised by a
// flow's own page loop: the commit's walk of the Library's listing, or a
// gateway paging through one of the provider's own, stopped at its cap. It is
// answered the way the flows' own caps are, from every flow that can carry it,
// and never as Storage not answering.
#[test]
fn a_listing_the_storage_port_says_ran_past_its_cap_is_answered_the_same_way() {
    let port = || coffret_usecase::Error::ListingPastCap {
        pages: 100_000,
        source: None,
    };
    let mut refusals = vec![
        (
            "sync",
            ApiError::from(Error::Sync {
                cause: SyncError::Storage(port()),
            }),
        ),
        (
            "freeze",
            ApiError::from(Error::Freeze {
                cause: FreezeError::Storage(port()),
            }),
        ),
        (
            "fetch",
            ApiError::from(Error::Fetch {
                cause: FetchError::Storage(port()),
            }),
        ),
    ];
    refusals.extend(from_every_flow(|| CommitError::Storage(port())));
    let flows_own = ApiError::from(Error::Sync {
        cause: SyncError::ListingLimitReached { pages: 100_000 },
    })
    .message()
    .to_owned();

    for (flow, refusal) in refusals {
        assert_eq!(refusal.message(), flows_own, "from a {flow}");
        assert_eq!(wire(refusal), (502, "storage", None, None), "from a {flow}");
    }
}

// ---------------------------------------------------------------------------
// What a refusal writes down.
//
// The routes' own cases plant a sentinel and read the log back over a real
// request, which is what states the rule end to end. These are the other half:
// the failures a route cannot easily be driven into — an Entry a Container the
// catalog names does not hold, two paths landing on one file — asked of the
// value directly, so that every variant carrying a path is covered rather than
// the two that are convenient to stage.
// ---------------------------------------------------------------------------

/// A second path, for the refusals that are about two.
fn other_path() -> EntryPath {
    entry_path("albums/SPRING.JPG")
}

/// A folder on this device, for the refusals that name one.
fn local_folder() -> PathBuf {
    PathBuf::from("/home/someone/albums")
}

/// What one refusal put into the log, and what it did not.
///
/// The capture is the thread's, so the record read back is this case's own.
fn recorded(refusal: ApiError) -> String {
    let logs = coffret_logging::testing::CapturedLogs::capture();
    refusal.record("case");

    let event = logs.only(tracing::Level::ERROR);
    let error = event.field("error");
    logs.assert_free_of(&[
        path().as_str(),
        other_path().as_str(),
        "spring",
        "SPRING",
        "albums",
        "someone",
    ]);
    error
}

// Every fetch refusal that is identified by an Entry Path, and the one that is
// identified by a local folder as well. Each is named to a person in its
// message and to the log by its shape (spec: EL-1).
#[test]
fn no_refusal_a_path_identifies_writes_the_path_down() {
    let cases = [
        (
            FetchError::UnmappedEntryPath { path: path() },
            "Fetch::UnmappedEntryPath(path_len=17)",
        ),
        (
            FetchError::UnmaterializablePath {
                path: path(),
                stopped_at: None,
            },
            "Fetch::UnmaterializablePath(path_len=17, descent=unspellable)",
        ),
        (
            FetchError::UnmaterializablePath {
                path: path(),
                stopped_at: Some(local_folder()),
            },
            "Fetch::UnmaterializablePath(path_len=17, descent=blocked)",
        ),
        (
            FetchError::LocalPathCollision {
                first: path(),
                second: other_path(),
            },
            "Fetch::LocalPathCollision(first_len=17, second_len=17)",
        ),
    ];
    for (cause, expected) in cases {
        assert_eq!(recorded(ApiError::from(Error::Fetch { cause })), expected);
    }
}

// EP-13, EL-1: what a refused root writes down is which of the seven cases it
// was and nothing else. Neither the folder nor either identity: a local path may
// not be written down, and the case is the whole of what somebody investigating
// one goes on. The mapping the *sentence* names is not here either — a prefix is
// a person-facing rendering and is not reused for an event — so the line is the
// same one it was before the sentence started naming it.
//
// The line is the reporting error's own rendering rather than one this route
// writes, so it also says which layer met the state: a fetch that stopped while
// placing, or this device placing the one file a drop handed it. One answer to
// the browser, two accounts in the log, because the two are investigated from
// different ends.
#[test]
fn a_refused_root_records_which_case_it_was_and_no_path() {
    assert_eq!(
        recorded(ApiError::from(Error::Fetch {
            cause: FetchError::RefusedRoot(RefusedRoot {
                prefix: Some(entry_path("albums")),
                local_root: local_folder(),
                reason: RootRefused::MarkerMismatch,
            }),
        })),
        "Fetch::RefusedRoot: MarkerMismatch",
    );
    assert_eq!(
        recorded(ApiError::from(Error::RootRefused(RefusedRoot {
            prefix: Some(entry_path("albums")),
            local_root: local_folder(),
            reason: RootRefused::MarkerMissing,
        }))),
        "Device::RootRefused: MarkerMissing",
        "the same state met by a drop, under the name of the layer that met it",
    );
}

// EP-14, EL-1: the component the refusal names is a piece of the Entry Path, so
// it is left out for the reason the path is — the length is what is left. Both
// readings of the reservation are recorded, and by their own names: which of the
// two a run met is the one thing about it a log may carry.
#[test]
fn a_reserved_component_writes_neither_the_path_nor_the_component() {
    assert_eq!(
        recorded(ApiError::from(Error::Fetch {
            cause: FetchError::ReservedComponent {
                path: path(),
                component: ".coffret".to_owned(),
            },
        })),
        "Fetch::ReservedComponent(path_len=17)",
    );
    assert_eq!(
        recorded(ApiError::from(Error::Fetch {
            cause: FetchError::FoldedReservedComponent {
                path: path(),
                component: ".COFFRET".to_owned(),
            },
        })),
        "Fetch::FoldedReservedComponent(path_len=17)",
    );
}

// The two integrity verdicts that name an Entry inside a Container. The
// Container stays — it is a name this Library minted, and it is what somebody
// investigating one of these goes and looks at — and the Entry Path does not.
#[test]
fn an_integrity_verdict_keeps_the_container_and_drops_the_entry_path() {
    for cause in [
        FetchError::EntryMissing {
            container_id: container_id(),
            path: path(),
        },
        FetchError::ContentMismatch {
            container_id: container_id(),
            path: path(),
        },
    ] {
        let error = recorded(ApiError::from(Error::Fetch { cause }));
        assert!(error.contains(&container_id().to_string()), "{error}");
        assert!(error.ends_with("path_len=17)"), "{error}");
    }
}

// The flows that walk the device's own folders reach the log through the same
// recording, and their collisions are identified by a path in just the same way.
//
// The device layer's own wrapper is not in the rendering, and that is the
// mapping above rather than a gap: a refusal is built from the flow's failure
// itself, having already read which flow it was.
#[test]
fn the_flows_that_walk_this_device_write_no_path_down_either() {
    assert_eq!(
        recorded(ApiError::from(Error::Sync {
            cause: SyncError::PathCollision { path: path() },
        })),
        "Sync::PathCollision(path_len=17)",
    );
    assert_eq!(
        recorded(ApiError::from(Error::CatchUp {
            cause: CommitError::EntryPathCollision { path: path() },
        })),
        "Commit::EntryPathCollision(path_len=17)",
    );
    // Classified as every commit's failure is, and written down as the flow
    // that met it reported it: which flow was committing is what somebody
    // reading the line starts from.
    assert_eq!(
        recorded(ApiError::from(Error::Sync {
            cause: SyncError::Commit(CommitError::EntryPathCollision { path: path() }),
        })),
        "Sync::Commit: Commit::EntryPathCollision(path_len=17)",
    );
}

// The device layer's own states, which is where its wrapper does show: nothing
// under a fetch, a sync, a freeze or a catch-up goes through here, and what
// does is this machine — its settings file, its catalog, its Library
// directory. None of those may be named either.
#[test]
fn the_devices_own_states_are_recorded_by_what_they_are() {
    assert_eq!(
        recorded(ApiError::from(Error::Index {
            cause: coffret_usecase::IndexError::NoCheckpoint,
        })),
        "Device::Index: Index::NoCheckpoint",
    );
}

// A refusal with nothing underneath it writes nothing: there is no failure to
// account for, and an event saying so would be one more line between a reader
// and the ones that mean something.
#[test]
fn a_refusal_with_no_failure_under_it_records_nothing() {
    let logs = coffret_logging::testing::CapturedLogs::capture();
    ApiError::no_such_entry().record("case");
    ApiError::declined(&Surfaced::ForeignFile { path: path() }).record("case");

    assert!(logs.at(tracing::Level::ERROR).is_empty(), "{}", logs.text());
}

/// Where the explorer reads the finding names from, relative to this crate.
///
/// One committed file rather than a build step, because the two sides are
/// built by different toolchains and a browser bundle has no way to ask a Rust
/// binary what it can say.
const SURFACED_FINDINGS: &str =
    "../../../../frontend/packages/gateway/api/src/surfaced-findings.json";

/// The next finding after `previous`, and `None` past the last of them.
///
/// A walk rather than a list, because a list is a thing to forget to add to.
/// The `match` is exhaustive, so a variant added to `Surfaced` fails to compile
/// here until it is given its place in the order — and the walk then visits it
/// without being told to, which is what puts its name in front of the case
/// below.
fn after(previous: Option<&Surfaced>) -> Option<Surfaced> {
    match previous {
        None => Some(Surfaced::ForeignFile { path: path() }),
        Some(Surfaced::ForeignFile { .. }) => Some(Surfaced::LocallyChanged { path: path() }),
        Some(Surfaced::LocallyChanged { .. }) => Some(Surfaced::WitnessedDeletion { path: path() }),
        Some(Surfaced::WitnessedDeletion { .. }) => Some(Surfaced::UnreachablePlace {
            path: path(),
            stopped_at: PathBuf::from("/home/someone/albums"),
        }),
        Some(Surfaced::UnreachablePlace { .. }) => Some(Surfaced::KeyLost {
            path: path(),
            container_id: container_id(),
        }),
        Some(Surfaced::KeyLost { .. }) => Some(Surfaced::ReservedComponent {
            path: entry_path("albums/.coffret/root"),
        }),
        Some(Surfaced::ReservedComponent { .. }) => None,
    }
}

/// Every name this server can put in a refusal's `surfaced` field, in order.
fn every_finding_name() -> Vec<&'static str> {
    let mut names = Vec::new();
    let mut current = after(None);
    while let Some(finding) = current {
        names.push(name_of(&finding));
        current = after(Some(&finding));
    }
    names
}

// EP-11: the explorer branches on these names, and a name it has never heard of
// reads as `null` — which every screen then shows as the generic sentence, with
// nothing anywhere saying that a finding went missing. So the names are not
// written down twice: the file the explorer imports is the one list, and this is
// what holds the server to it. Renaming a variant fails here until the file is
// brought along.
//
// The file is where the checking stops, though: it is an array of strings, so
// the `SurfacedFinding` union the explorer's callers branch on is not held to it
// by anything a compiler runs — a name in the file that the union has never
// heard of is cast into it and reaches a `switch` with no case for it. That
// third step is the one a person has to take, so the message below asks for it
// rather than leaving `cargo test` pointing only at the file.
#[test]
fn the_findings_file_the_explorer_reads_holds_the_names_this_server_sends() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(SURFACED_FINDINGS);
    let held = fs::read_to_string(&path)
        .unwrap_or_else(|cause| panic!("{} must be readable: {cause}", path.display()));
    let held: Vec<String> = serde_json::from_str(&held)
        .unwrap_or_else(|cause| panic!("{} must be an array of names: {cause}", path.display()));

    assert_eq!(
        held,
        every_finding_name(),
        "{} has fallen behind `name_of`; write these names into it, in this order, and bring \
         the `SurfacedFinding` union in refusal.ts beside it — the file holds strings, so \
         nothing on that side fails when the two disagree",
        path.display(),
    );
}
