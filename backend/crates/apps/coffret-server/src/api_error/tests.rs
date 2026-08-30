use coffret_device::{EntryPath, Error, FetchError, Surfaced};
use coffret_model::{ContainerId, ContentHash};

use super::ApiError;

/// The path every case here refuses something about.
fn path() -> EntryPath {
    EntryPath::nfc("albums/spring.jpg")
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
#[test]
fn a_path_this_device_cannot_hold_a_file_at_is_declined_as_unmaterializable() {
    assert_eq!(
        from(FetchError::UnmaterializablePath { path: path() }),
        (409, "declined", Some("unmaterializable"), None),
    );
    assert_eq!(
        from(FetchError::LocalPathCollision {
            first: path(),
            second: EntryPath::nfc("albums/SPRING.JPG"),
        }),
        (409, "declined", Some("unmaterializable"), None),
    );
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
