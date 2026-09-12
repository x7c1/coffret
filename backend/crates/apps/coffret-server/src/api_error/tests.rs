use std::path::PathBuf;

use coffret_device::{EntryPath, Error, FetchError, RootRefused, Surfaced};
use coffret_model::{ContainerId, ContentHash};
use coffret_usecase::root_marker::MalformedMarker;

use super::ApiError;
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

// EP-13: a mapped folder that is not the folder its mapping was recorded
// against is this device's configuration rather than the server failing, so
// every one of the seven cases reaches the browser as one declined answer with
// a reason of its own — never as the `500` that says only that the server could
// not answer. One sentence for all seven, because the gesture is one gesture.
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
        // Both ways one reaches a route: a fetch that met it while placing, and
        // this device placing the one file an upload handed it.
        let from_fetch = ApiError::from(Error::Fetch {
            cause: FetchError::RefusedRoot {
                local_root: PathBuf::from("/mnt/copied"),
                reason: reason.clone(),
            },
        });
        let from_upload = ApiError::from(Error::RootRefused {
            root: PathBuf::from("/mnt/copied"),
            reason: reason.clone(),
        });

        for refusal in [from_fetch, from_upload] {
            let message = refusal.message().to_owned();
            assert_eq!(
                wire(refusal),
                (409, "declined", Some("refused_root"), None),
                "{reason:?}",
            );
            assert!(
                message.contains("coffret map"),
                "the sentence names the one gesture that settles it: {message}",
            );
            assert!(
                !message.contains("copied"),
                "and never the folder, which is a local path: {message}",
            );
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
        // EP-14: the path carries the name of the device's own folder, which
        // is a finding about one Entry and never a place a file is put.
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

// EP-13, EL-1: a refused root is the one refusal on these routes whose body says
// nothing about what it met, so the log is the only account of it — and what goes
// there is which of the seven cases it was and nothing else. Neither the folder
// nor either identity: a local path may not be written down, and the case is the
// whole of what somebody investigating one goes on.
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
            cause: FetchError::RefusedRoot {
                local_root: local_folder(),
                reason: RootRefused::MarkerMismatch,
            },
        })),
        "Fetch::RefusedRoot: MarkerMismatch",
    );
    assert_eq!(
        recorded(ApiError::from(Error::RootRefused {
            root: local_folder(),
            reason: RootRefused::MarkerMissing,
        })),
        "Device::RootRefused: MarkerMissing",
        "the same state met by a drop, under the name of the layer that met it",
    );
}

// EP-14, EL-1: the component the refusal names is a piece of the Entry Path, so
// it is left out for the reason the path is — the length is what is left.
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
            cause: coffret_usecase::sync::SyncError::PathCollision { path: path() },
        })),
        "Sync::PathCollision(path_len=17)",
    );
    assert_eq!(
        recorded(ApiError::from(Error::CatchUp {
            cause: coffret_device::CommitError::EntryPathCollision { path: path() },
        })),
        "Commit::EntryPathCollision(path_len=17)",
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
