//! What each [`Error`] variant says, what it carries under that, and what a
//! diagnostic event is told of it.

use coffret_model::Redacted;
use coffret_usecase::fetch::FetchError;
use coffret_usecase::root_marker::{MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::RootRefused;

use super::*;
use crate::testing::entry_path;

/// The links a caller printing `{error:#}` reads, outermost first.
fn chain(error: &dyn error::Error) -> Vec<String> {
    let mut links = vec![error.to_string()];
    let mut below = error.source();
    while let Some(link) = below {
        links.push(link.to_string());
        below = link.source();
    }
    links
}

// The message names the Library and the directory it is in, which is what
// the person standing at this device needs; the diagnostic event names
// the state and nothing they called anything.
#[test]
fn a_library_that_is_already_here_is_recorded_without_its_name() {
    let error = Error::LibraryExists {
        name: "holiday-photos".to_owned(),
        path: PathBuf::from("/home/someone/.local/state/coffret/holiday-photos"),
    };

    assert!(error.to_string().contains("holiday-photos"));
    assert_eq!(error.redacted(), "Device::LibraryExists");
}

// LA-8 as EL-1 sees it. The person starting a second server is told which
// Library of theirs it is about and which process to stop; the diagnostic
// event keeps the process and none of the name.
#[test]
fn a_library_already_being_served_is_recorded_without_its_name() {
    let error = Error::LibraryAlreadyServed {
        name: "holiday-photos".to_owned(),
        by: Some(4213),
    };

    let said = error.to_string();
    assert!(said.contains("holiday-photos"), "{said}");
    assert!(said.contains("4213"), "{said}");
    assert_eq!(error.redacted(), "Device::LibraryAlreadyServed(by=4213)");

    // A server killed between taking the lock and writing its number down
    // leaves the sentence one clause shorter rather than no refusal at all:
    // what says a server is there is the lock, not the number.
    let anonymous = Error::LibraryAlreadyServed {
        name: "holiday-photos".to_owned(),
        by: None,
    };

    let said = anonymous.to_string();
    assert!(said.contains("holiday-photos"), "{said}");
    assert!(!said.contains("process"), "{said}");
    assert_eq!(
        anonymous.redacted(),
        "Device::LibraryAlreadyServed(by=unknown)"
    );
}

// The entropy source's refusal travels as the value it reported, so a
// caller can read the kind it named off the chain. That kind is one of the
// few things this vocabulary may write down, since it is about this
// machine's random source and nothing anybody chose.
#[test]
fn a_server_key_that_could_not_be_drawn_carries_what_the_source_reported() {
    use std::error::Error as _;

    let reported = getrandom::Error::UNSUPPORTED;
    let error = Error::ServerKeyNotDrawn { cause: reported };

    let source = error.source().expect("the chain reaches the source");
    assert!(
        source.downcast_ref::<getrandom::Error>().is_some(),
        "the source is the value getrandom reported and not a rendering of it",
    );
    // Said once: this line names what could not be drawn, and what the
    // source reported is the link under it.
    assert_eq!(
        chain(&error),
        vec![
            "the key this server would admit its callers by could not be drawn".to_owned(),
            reported.to_string(),
        ],
    );
    // Composed from the source's own rendering rather than written out: a
    // reworded upstream sentence is not this layer's rendering changing. A
    // diagnostic event has no chain to walk, so this is the one rendering
    // that still spells the cause out.
    assert_eq!(
        error.redacted(),
        format!("Device::ServerKeyNotDrawn: {reported}"),
    );
}

// The chain a refusal reaches the log as, whole: which flow, which refusal
// inside it, and the shape of the refusal — and no path from either end.
#[test]
fn a_fetch_that_could_not_place_a_file_records_the_whole_chain() {
    let error = Error::Fetch {
        cause: Box::new(FetchError::UnmaterializablePath {
            path: entry_path("albums/spring.jpg"),
            stopped_at: Some(PathBuf::from("/home/someone/albums")),
        }),
    };

    assert_eq!(
        error.redacted(),
        "Device::Fetch: Fetch::UnmaterializablePath(path_len=17, descent=blocked)",
    );
}

// A refusal about where a file belongs is not a refusal about a fetch, and
// the chain says which it is at its own outermost link: nothing was
// transferred, so a first sentence about a transfer would send whoever reads
// it looking for one.
#[test]
fn a_path_that_could_not_be_placed_says_so_without_naming_a_fetch() {
    let error = Error::LocalPathNotSettled {
        cause: Box::new(FetchError::UnmappedEntryPath {
            path: entry_path("albums/spring.jpg"),
        }),
    };

    assert_eq!(
        chain(&error),
        vec![
            "where on this device that file belongs was not settled".to_owned(),
            FetchError::UnmappedEntryPath {
                path: entry_path("albums/spring.jpg"),
            }
            .to_string(),
        ],
    );
    assert_eq!(
        error.redacted(),
        "Device::LocalPathNotSettled: Fetch::UnmappedEntryPath(path_len=17)",
    );
}

// The other half of the same rule, for the gesture that has a file in hand:
// a drop that was turned away is answered as a drop. The vocabulary inside
// the chain is the fetch's because the rule about where a file may stand on
// this device is written once, and the sentence on the outside is about the
// file, which is what the person did.
#[test]
fn a_file_that_was_not_taken_in_says_so_without_naming_a_fetch() {
    let error = Error::FileNotTakenIn {
        cause: Box::new(FetchError::UnmappedEntryPath {
            path: entry_path("albums/spring.jpg"),
        }),
    };

    assert_eq!(
        chain(&error),
        vec![
            "the file was not taken in".to_owned(),
            FetchError::UnmappedEntryPath {
                path: entry_path("albums/spring.jpg"),
            }
            .to_string(),
        ],
    );
    assert_eq!(
        error.redacted(),
        "Device::FileNotTakenIn: Fetch::UnmappedEntryPath(path_len=17)",
    );
}

// And the third gesture, which has neither a transfer nor a file in hand:
// somebody looking at what is in a folder of their own. The refusal kept
// for a name a case-folding volume will not tell apart from this device's
// management area is the one that reaches a reader, and it is refused
// before any mapping is read — so the outer sentence is about the answer
// that did not come back rather than about a path that was not settled.
#[test]
fn a_read_of_a_mapped_folder_says_what_was_not_read_without_naming_a_fetch() {
    let error = Error::LocalFilesNotRead {
        cause: Box::new(FetchError::FoldedReservedComponent {
            path: entry_path("albums/.COFFRET"),
            component: ".COFFRET".to_owned(),
        }),
    };

    assert_eq!(
        chain(&error),
        vec![
            "what this device has of its own there was not read".to_owned(),
            FetchError::FoldedReservedComponent {
                path: entry_path("albums/.COFFRET"),
                component: ".COFFRET".to_owned(),
            }
            .to_string(),
        ],
    );
    assert_eq!(
        error.redacted(),
        "Device::LocalFilesNotRead: Fetch::FoldedReservedComponent(path_len=15)",
    );
}

// And the fourth, which is the one a browser walks over every time somebody
// opens a picture: the file this device placed for an Entry the Library
// holds. The refusal chosen here is the one that would read worst under the
// other sentences — the Library no longer holds the Entry, so nothing was
// unsettled about a path and nothing of this device's own was being looked
// at — and it is a state the caller goes on from rather than fails at
// (spec: EP-10).
#[test]
fn a_placed_file_that_did_not_open_says_so_without_naming_a_fetch() {
    let error = Error::LocalFileNotOpened {
        cause: Box::new(FetchError::EntryNotCurrent {
            path: entry_path("albums/spring.jpg"),
        }),
    };

    assert_eq!(
        chain(&error),
        vec![
            "what this device has for that Entry was not opened".to_owned(),
            FetchError::EntryNotCurrent {
                path: entry_path("albums/spring.jpg"),
            }
            .to_string(),
        ],
    );
    assert_eq!(
        error.redacted(),
        "Device::LocalFileNotOpened: Fetch::EntryNotCurrent(path_len=17)",
    );
}

// EL-1: the person standing at the device is told which file refused, since
// that is the one thing they can go and look at; the diagnostic event
// carries the operation and the kind of refusal and no part of the path.
// The refusal travels whole, so its own `io::Error` is still the chain's
// next link.
#[test]
fn a_local_refusal_names_the_file_for_a_person_and_never_for_the_log() {
    use std::error::Error as _;

    let error = Error::Local(LocalIoError::new(
        LocalOperation::Renaming,
        PathBuf::from("/home/someone/albums/spring.jpg"),
        io::Error::from(io::ErrorKind::PermissionDenied),
    ));

    assert!(
        error
            .to_string()
            .contains("/home/someone/albums/spring.jpg"),
        "{error}",
    );
    assert_eq!(
        error.redacted(),
        "Device::Local: Local::Io(operation=renamed, kind=PermissionDenied)",
    );

    // And the operating system's answer is said once, underneath, rather
    // than copied into the line above it as well.
    let answered = io::Error::from(io::ErrorKind::PermissionDenied).to_string();
    assert!(
        !error.to_string().contains(&answered),
        "the cause belongs under this line and not inside it: {error}",
    );
    assert_eq!(
        error.source().map(ToString::to_string),
        Some(answered),
        "the cause is still reachable",
    );
}

// EL-1, EL-2: every coffret error in a nested cause chain contributes its
// log-safe rendering. The device-local Library name, the path derived from
// it, and the foreign I/O message remain available to a person and absent
// from the diagnostic form.
#[test]
fn a_nested_creation_failure_keeps_no_device_local_library_name() {
    const LIBRARY: &str = "Family Tax Records";
    const PRIVATE_PATH: &str =
        "/Users/alice/Library/Application Support/coffret/libraries/Family Tax Records.partial";
    let error = Error::LibraryNotCreated {
        name: LIBRARY.to_owned(),
        step: CreationStep::Publish,
        orphan_folder: None,
        cause: Box::new(Error::Local(LocalIoError::new(
            LocalOperation::Renaming,
            PRIVATE_PATH,
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("rename of {PRIVATE_PATH} was denied"),
            ),
        ))),
    };

    let rendered = error.redacted();
    assert_eq!(
        rendered,
        "Device::LibraryNotCreated(step=Publish, orphan_folder=false): \
         Device::Local: Local::Io(operation=renamed, kind=PermissionDenied)"
    );
    assert!(!rendered.contains(LIBRARY), "{rendered}");
    assert!(!rendered.contains(PRIVATE_PATH), "{rendered}");
    assert!(error.to_string().contains(LIBRARY), "{error}");
}

// The one refusal a shell has words of its own for, as it actually
// arrives: wrapped twice, the creation step over the Drive refusal. The
// shell only ever holds the outer one, so the look-through is what decides
// whether the variable gets named at all.
#[test]
fn a_creation_refused_at_the_exchange_without_a_secret_is_seen_through_its_wrappers() {
    let sent_no_secret = || Error::Drive {
        cause: Box::new(google_drive_store::Error::CodeExchangeWithoutSecret {
            status: 401,
            detail: r#"{"error":"invalid_client"}"#.to_owned(),
        }),
    };

    let created = Error::LibraryNotCreated {
        name: "holiday-photos".to_owned(),
        step: CreationStep::Authorization,
        orphan_folder: None,
        cause: Box::new(sent_no_secret()),
    };
    assert!(created.is_exchange_without_client_secret(), "{created}");

    let joined = Error::LibraryNotJoined {
        name: "holiday-photos".to_owned(),
        step: CreationStep::Authorization,
        cause: Box::new(sent_no_secret()),
    };
    assert!(joined.is_exchange_without_client_secret(), "{joined}");
}

// And the endpoint's own refusal of a client that did send its secret
// answers no: that run was refused for some other reason, and naming the
// variable would send somebody to change the one thing that was not the
// matter.
#[test]
fn a_creation_refused_for_another_reason_says_nothing_about_a_secret() {
    let refused = Error::LibraryNotCreated {
        name: "holiday-photos".to_owned(),
        step: CreationStep::Authorization,
        orphan_folder: None,
        cause: Box::new(Error::Drive {
            cause: Box::new(google_drive_store::Error::TokenEndpoint {
                status: 401,
                detail: r#"{"error":"invalid_grant"}"#.to_owned(),
            }),
        }),
    };
    assert!(!refused.is_exchange_without_client_secret(), "{refused}");
}

// EP-2: a prefix that is no Entry Path is refused in the model's words, and
// those words are printed once. The head line says what this layer knows —
// which prefix, and that nothing was mapped — because a shell printing the
// chain prints the cause under it and would otherwise say it twice.
#[test]
fn a_prefix_that_is_no_entry_path_says_the_reason_once() {
    use std::error::Error as _;

    let error = Error::MalformedMappingPrefix {
        prefix: "albums/".to_owned(),
        cause: Some(coffret_model::Error::MalformedEntryPath {
            path: "albums/".to_owned(),
            defect: coffret_model::PathDefect::TrailingSeparator,
        }),
    };

    assert_eq!(error.to_string(), "\"albums/\" cannot be mapped");
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("\"albums/\" is not an Entry Path: it ends with a separator"),
    );
}

// EP-13: the marker's refusal reaches a person as one sentence per layer —
// this crate's, which names the folder and the gesture; the reading's,
// which says the content is no spelling of an identity; and the model's,
// which says what that spelling would have had to be.
#[test]
fn a_malformed_marker_reaches_a_caller_as_one_sentence_per_layer() {
    const ROOT: &str = "/home/someone/Pictures/Holidays";
    let error = Error::MarkerMalformed {
        root: PathBuf::from(ROOT),
        cause: coffret_usecase::root_marker::parse(b"not an identity")
            .expect_err("that content names no identity"),
    };

    assert_eq!(
        chain(&error),
        vec![
            format!(
                "{MANAGEMENT_AREA}/{MARKER_FILE} in {ROOT} names no identity; nothing was \
                 written and nothing was recorded, and a new identity asked for replaces \
                 one rather than repairing this"
            ),
            "a marker's content is the spelling of an identity and this is not".to_owned(),
            "not the 16 lowercase hexadecimal characters a root's identity is spelled as"
                .to_owned(),
            "expected 16 hex characters, found 15".to_owned(),
        ],
    );
}

// EL-1, EP-13: a mapped root is a folder somebody keeps their own files in,
// so every refusal registration makes tells the person standing at the
// device which folder to go and look at, and tells the diagnostic event
// only which state of coffret's own folder it was.
#[test]
fn the_marker_refusals_name_the_root_for_a_person_and_never_for_the_log() {
    const ROOT: &str = "/home/someone/Pictures/Holidays";
    // A folder of the person's own that a case-folding volume does not tell
    // apart from coffret's (spec: EP-14), for the one refusal that names
    // such a folder as well as the root.
    const FOLDED: &str = ".COFFRET";

    // What the entropy source said, for the one refusal that carries such a
    // cause. Its rendering is composed from the constant rather than written
    // out, so a reworded upstream sentence is not a failure of this layer.
    let unavailable = getrandom::Error::UNSUPPORTED;

    let refusals = [
        (
            Error::ManagementAreaNotADirectory {
                root: PathBuf::from(ROOT),
            },
            "Device::ManagementAreaNotADirectory".to_owned(),
        ),
        (
            Error::ManagementAreaIncomplete {
                root: PathBuf::from(ROOT),
            },
            "Device::ManagementAreaIncomplete".to_owned(),
        ),
        // The one of them about a folder of the person's rather than
        // coffret's, so it names two things to them — the root and the
        // folder standing in it — and neither reaches the event.
        (
            Error::ManagementAreaFolded {
                root: PathBuf::from(ROOT),
                name: FOLDED.to_owned(),
            },
            "Device::ManagementAreaFolded".to_owned(),
        ),
        (
            Error::MarkerNotARegularFile {
                root: PathBuf::from(ROOT),
            },
            "Device::MarkerNotARegularFile".to_owned(),
        ),
        (
            Error::MarkerMalformed {
                root: PathBuf::from(ROOT),
                // Through the reading itself, which is the only thing that
                // makes one of these: what is wrong with the content is not
                // a shape this layer gets to state.
                cause: coffret_usecase::root_marker::parse(b"not an identity")
                    .expect_err("that content names no identity"),
            },
            "Device::MarkerMalformed(defect=not an identity)".to_owned(),
        ),
        (
            Error::RootMarkerNotDrawn {
                root: PathBuf::from(ROOT),
                cause: coffret_format::Error::EntropyUnavailable { cause: unavailable },
            },
            format!(
                "Device::RootMarkerNotDrawn: Format: could not draw random bytes: \
                 {unavailable}"
            ),
        ),
        // The one the marker's *reader* makes rather than its writer, and it
        // owes the same two things: the folder to the person, and the shape
        // of the wrong folder to the event.
        (
            Error::RootRefused(RefusedRoot {
                prefix: Some(entry_path("albums")),
                local_root: PathBuf::from(ROOT),
                reason: RootRefused::MarkerMismatch,
            }),
            "Device::RootRefused: MarkerMismatch".to_owned(),
        ),
    ];

    for (error, rendered) in refusals {
        let said = error.to_string();
        assert!(
            said.contains(ROOT),
            "the person is told which folder it is about: {said}"
        );
        assert_eq!(error.redacted(), rendered);
        assert!(
            !error.redacted().contains(ROOT),
            "and the event carries no part of it: {}",
            error.redacted()
        );
        assert!(
            !error.redacted().contains(FOLDED),
            "nor any name standing in it: {}",
            error.redacted()
        );
    }
}

// EL-1, EP-13: the reading's other outcome, where the marker settles nothing
// because the operating system would not answer. The person is owed the same
// two things as above and one more — which of coffret's own names inside the
// folder refused. A reading passes through two of them, and they are
// different things to go and look at: a sentence that named the marker for a
// refusal met on the folder holding it would send somebody to a file whose
// own mode is sound. The event carries the operation and the kind and no
// part of either path.
#[test]
fn a_root_that_could_not_be_asked_about_names_what_refused_and_never_the_folder() {
    const ROOT: &str = "/home/someone/Pictures/Holidays";

    let refused = |operation, path: String| Error::RootUnvouched {
        local_root: PathBuf::from(ROOT),
        cause: LocalIoError::new(
            operation,
            path,
            io::Error::from(io::ErrorKind::PermissionDenied),
        ),
    };

    let area = refused(LocalOperation::Stating, format!("{ROOT}/{MANAGEMENT_AREA}"));
    let said = area.to_string();
    assert!(
        said.contains(ROOT),
        "the person is told which folder it is about: {said}",
    );
    assert!(
        said.contains(&format!("{MANAGEMENT_AREA} in it could not be stated")),
        "the folder holding the marker is what refused, and it is what is named: {said}",
    );
    assert!(
        !said.contains(&format!("{MANAGEMENT_AREA}/{MARKER_FILE}")),
        "nothing was asked of the marker itself here: {said}",
    );

    let marker = refused(
        LocalOperation::Reading,
        format!("{ROOT}/{MANAGEMENT_AREA}/{MARKER_FILE}"),
    );
    assert!(
        marker.to_string().contains(&format!(
            "{MANAGEMENT_AREA}/{MARKER_FILE} in it could not be read"
        )),
        "and the marker where that is what refused: {marker}",
    );

    assert_eq!(
        marker.redacted(),
        "Device::RootUnvouched: Local::Io(operation=read, kind=PermissionDenied)",
    );
    assert!(
        !marker.redacted().contains(ROOT),
        "the event carries no part of it: {}",
        marker.redacted(),
    );
}

// SA-8, EL-1: every refusal about an account says which one to the person
// reading it, in one sentence each, and none of them tells a diagnostic event
// the account's name, the Library's, or either client.
#[test]
fn the_account_refusals_name_the_account_for_a_person_and_never_for_the_log() {
    const ACCOUNT: &str = "family_photos";
    const LIBRARY: &str = "Summer 2026";
    let cases = [
        (
            Error::InvalidAccountName {
                name: "family/photos".to_owned(),
            },
            "\"family/photos\" cannot name an account: an account name is 1 to 64 characters, \
             each an ASCII letter, a digit, '-' or '_'",
            "Device::InvalidAccountName",
        ),
        (
            Error::AccountNotOpened {
                account: ACCOUNT.to_owned(),
                library: LIBRARY.to_owned(),
            },
            "the account \"family_photos\" opens only through a Library that references it, \
             and the Passphrase given does not open one; the Passphrase of the Library \
             \"Summer 2026\" does",
            "Device::AccountNotOpened",
        ),
        (
            Error::NoSuchAccount {
                account: ACCOUNT.to_owned(),
            },
            "no account \"family_photos\" is on this device",
            "Device::NoSuchAccount",
        ),
        (
            Error::AccountFixed {
                library: LIBRARY.to_owned(),
                account: ACCOUNT.to_owned(),
                requested: "work".to_owned(),
            },
            "the Library \"Summer 2026\" references the account \"family_photos\", not \
             \"work\", and the account a Library references cannot be changed yet; run \
             `coffret authorize --library Summer 2026` to renew that account's grant",
            "Device::AccountFixed",
        ),
        (
            Error::UnreadableAccountEnvelope {
                library: LIBRARY.to_owned(),
                account: ACCOUNT.to_owned(),
                cause: None,
            },
            "the envelope that opens the account \"family_photos\" for the Library \
             \"Summer 2026\" is missing; nothing was renewed and no consent was asked for",
            "Device::UnreadableAccountEnvelope(missing)",
        ),
        (
            Error::ClientMismatch(Box::new(ClientMismatch {
                library: LIBRARY.to_owned(),
                library_client: "one.apps.googleusercontent.com".to_owned(),
                account: ACCOUNT.to_owned(),
                account_client: "two.apps.googleusercontent.com".to_owned(),
            })),
            "the Library \"Summer 2026\" names the OAuth client \
             \"one.apps.googleusercontent.com\" and the account \"family_photos\" names \
             \"two.apps.googleusercontent.com\"; an account's grant is used only through the \
             client it was consented to; give --account a new name to consent through the \
             Library's client as another account",
            "Device::ClientMismatch",
        ),
        (
            Error::UnreadableAccountEnvelope {
                library: LIBRARY.to_owned(),
                account: ACCOUNT.to_owned(),
                cause: Some(Box::new(
                    coffret_format::Error::AccountCacheKeyEnvelopeLength { actual: 15 },
                )),
            },
            "the envelope that opens the account \"family_photos\" for the Library \
             \"Summer 2026\" could not be read; nothing was renewed and no consent was asked for",
            "Device::UnreadableAccountEnvelope: Format: an account-cache key envelope is 79 bytes \
             long, not 15",
        ),
        (
            Error::NoAccountReachesFolder,
            "none of the accounts this device holds reaches that folder, and a new account needs \
             a name while this device holds any: give --account with a name for the account the \
             folder is in",
            "Device::NoAccountReachesFolder",
        ),
        (
            Error::PromotionNeedsName {
                library: LIBRARY.to_owned(),
                account: ACCOUNT.to_owned(),
                obstacle: PromotionObstacle::ClientDiffers(Box::new(ClientMismatch {
                    library: LIBRARY.to_owned(),
                    library_client: "one.apps.googleusercontent.com".to_owned(),
                    account: ACCOUNT.to_owned(),
                    account_client: "two.apps.googleusercontent.com".to_owned(),
                })),
            },
            "the Library \"Summer 2026\" keeps a grant of its own from an earlier build, and the \
             account \"family_photos\" this device already holds cannot take it in; name an \
             account for it with `coffret authorize --library Summer 2026 --account NAME`",
            "Device::PromotionNeedsName(client-differs)",
        ),
        (
            Error::PromotionNeedsName {
                library: LIBRARY.to_owned(),
                account: ACCOUNT.to_owned(),
                obstacle: PromotionObstacle::NotOpened,
            },
            "the Library \"Summer 2026\" keeps a grant of its own from an earlier build, and the \
             account \"family_photos\" this device already holds cannot take it in; name an \
             account for it with `coffret authorize --library Summer 2026 --account NAME`",
            "Device::PromotionNeedsName(not-opened)",
        ),
        (
            Error::PromotionNeedsName {
                library: LIBRARY.to_owned(),
                account: ACCOUNT.to_owned(),
                obstacle: PromotionObstacle::FolderNotReached,
            },
            "the Library \"Summer 2026\" keeps a grant of its own from an earlier build, and the \
             account \"family_photos\" this device already holds cannot take it in; name an \
             account for it with `coffret authorize --library Summer 2026 --account NAME`",
            "Device::PromotionNeedsName(folder-not-reached)",
        ),
        (
            Error::AccountNameRequired { held: 3 },
            "this device holds 3 accounts, so which one the Library uses has to be named: give \
             --account with the name of one of them, or a new name to consent as another",
            "Device::AccountNameRequired(held=3)",
        ),
    ];
    for (error, said, recorded) in cases {
        assert_eq!(error.to_string(), said);
        assert_eq!(error.redacted(), recorded);
        for private in [ACCOUNT, LIBRARY, "family/photos", "googleusercontent"] {
            assert!(
                !error.redacted().contains(private),
                "{private:?} reached {}",
                error.redacted()
            );
        }
    }
}

// What boxing the flows' verdicts bought, held so that the next variant to
// carry something wide by value is noticed where it is added rather than
// paid for by every `Result` this crate returns. The number is where the
// widest variant still held inline lands — a bucket's name beside the
// Storage port's own refusal — and not a figure to defend for its own
// sake: a change that moves it on purpose moves it here, and says why.
// Pointer-sized fields make it a 64-bit figure.
#[cfg(target_pointer_width = "64")]
#[test]
fn a_device_error_is_no_wider_than_its_widest_inline_variant() {
    assert_eq!(std::mem::size_of::<Error>(), 88);
}
