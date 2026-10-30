//! Every state the work answer can be in, written out as the wire carries
//! it, for the explorer's own cases to read back through its types.
//!
//! The companion of the refusals' file (see `api_error`'s `contract`), and for
//! the reason that one gives: the two sides are built by different toolchains,
//! so a committed file between them is what holds one to the other. Built from
//! the values the flows publish rather than driven through the routes, because
//! what a screen branches on is every status of every flow, every phase, every
//! standing of the catalog and both states of the Library — and most of those
//! last a tick on a live server. The serialization is the route's own: these
//! are the same DTOs, built by the same `of` functions the route calls.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use coffret_device::{
    ByteCount, FindingReason, Generation, KeptTally, Phase, RootRefused, RootUnavailable,
    ShortKeyring, Step, Surfaced as DeviceSurfaced, UnheldParcel, UnheldReason,
};
use coffret_model::ContainerId;

use super::{
    CatalogDto, DeleteDto, FillDto, FreezeDto, KeptParcelsDto, ReconnectDto, SyncDto, WorkDto,
};
use crate::activity_log::{Ended, Logged, Queue};
use crate::api_error::{held_to, ApiError};
use crate::delete::{DeleteReport, DeleteRun, DeleteStatus, RefusedPack, Target};
use crate::displaced::Displaced;
use crate::entry_paths::entry_path;
use crate::fill::{Declined, FillRun, FillStatus};
use crate::finding::Finding;
use crate::folder::Folder;
use crate::freeze::{FreezeRun, FreezeStatus};
use crate::latest::Latest;
use crate::pending_rows::TurnTaker;
use crate::reconnect::Reconnect;
use crate::refresh::Standing;
use crate::reported::Reported;
use crate::sync::{SyncRun, SyncStatus};

/// Where the explorer reads the work answers from, relative to this crate.
const WORK: &str = "../../../../frontend/packages/gateway/api/src/contract/work.json";

/// What every answer here calls the process that gave it.
///
/// A constant rather than one drawn: the real name is entropy, and a file that
/// changed on every run could hold nothing to anything.
const SERVER: &str = "contract-server";

fn folder(path: &str) -> Folder {
    Folder::named((!path.is_empty()).then(|| entry_path(path)))
}

/// A refusal as a run's `stopped` carries it: Storage no longer taking the
/// grant, which is the one that carries a reason of its own.
fn storage() -> Reported {
    Reported::of(&ApiError::from(coffret_device::Error::Fetch {
        cause: Box::new(coffret_device::FetchError::Storage(
            coffret_usecase::Error::Unauthenticated {
                detail: "the grant has run out".to_owned(),
                source: None,
            },
        )),
    }))
}

/// Every finding a run can report, as the browser is told it.
fn every_finding() -> Vec<Finding> {
    use coffret_device::Finding as Found;

    let surfaced = |reason| Found::Surfaced {
        path: entry_path("albums/a.jpg"),
        reason,
    };
    let unavailable = |reason| Found::UnavailableRoot {
        prefix: Some(entry_path("albums")),
        local_root: PathBuf::from("/mnt/albums"),
        reason,
    };
    [
        surfaced(FindingReason::ForeignFile),
        surfaced(FindingReason::LocallyChanged),
        surfaced(FindingReason::WitnessedDeletion),
        surfaced(FindingReason::UnreachablePlace {
            stopped_at: PathBuf::from("/mnt/albums"),
        }),
        surfaced(FindingReason::KeyLost),
        surfaced(FindingReason::ReservedComponent),
        surfaced(FindingReason::ChangedInPack),
        surfaced(FindingReason::DeletedLocally),
        surfaced(FindingReason::Discarded),
        surfaced(FindingReason::KeptEdited),
        surfaced(FindingReason::DiscardRefused {
            cause: std::sync::Arc::new(coffret_device::DescentError::Io(
                coffret_usecase::LocalIoError::new(
                    coffret_usecase::LocalOperation::Discarding,
                    "/mnt/albums/a.jpg",
                    std::io::Error::other("no trash on this volume"),
                ),
            )),
        }),
        Found::Surfaced {
            path: entry_path("albums/b.jpg"),
            reason: FindingReason::Relocated {
                from: entry_path("inbox/b.jpg"),
            },
        },
        Found::Surfaced {
            path: entry_path("inbox/c.jpg"),
            reason: FindingReason::RelocationDiscarded {
                to: entry_path("albums/c.jpg"),
            },
        },
        Found::Surfaced {
            path: entry_path("inbox/d.jpg"),
            reason: FindingReason::RelocationRefused {
                to: entry_path("albums/d.jpg"),
                reason: coffret_device::RelocationRefused::DestinationTaken,
            },
        },
        Found::Surfaced {
            path: entry_path("albums/e.jpg"),
            reason: FindingReason::Renamed {
                from: entry_path("inbox/e.jpg"),
            },
        },
        Found::Surfaced {
            path: entry_path("x/f.jpg"),
            reason: FindingReason::RenameRefused {
                from: entry_path("inbox/f.jpg"),
                refusal: coffret_device::RenameRefusal::FileAndFolder {
                    path: entry_path("x"),
                },
            },
        },
        Found::Surfaced {
            path: entry_path("scans/vol-1/page-001.jpg"),
            reason: FindingReason::AwaitsPack,
        },
        Found::Surfaced {
            path: entry_path("scans/vol-1/page-002.jpg"),
            reason: FindingReason::Unreserved {
                lapse: coffret_device::Lapse::Gone,
            },
        },
        unavailable(RootUnavailable::Missing),
        unavailable(RootUnavailable::AnotherFilesystem),
        Found::RefusedRoot {
            prefix: None,
            local_root: PathBuf::from("/mnt/copied"),
            reason: RootRefused::MarkerMismatch,
        },
        Found::KeyLostContainer {
            container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
        },
        Found::ShortKeyring {
            generation: Generation::FIRST,
            replicas: 3,
            lost: 1,
            unfetched: 0,
        },
        Found::ShortKeyring {
            generation: Generation::FIRST,
            replicas: 3,
            lost: 0,
            unfetched: 1,
        },
        Found::KeyringRepaired {
            generation: Generation::FIRST,
            rewritten: NonZeroUsize::MIN,
        },
    ]
    .iter()
    .map(|found| Finding::of(found).expect("every finding here is one a browser is told"))
    .collect()
}

/// The finding a run that stopped still carries: the Keyring repair its
/// commit performed before it failed.
fn repaired_after_all() -> Finding {
    Finding::of(&coffret_device::Finding::KeyringRepaired {
        generation: Generation::FIRST,
        rewritten: NonZeroUsize::new(2).expect("two is not zero"),
    })
    .expect("a repair is one a browser is told")
}

/// A step in `phase`, counted or not, with bytes where the phase counts them.
fn step(phase: Phase, total: Option<usize>) -> Step {
    Step {
        phase,
        done: 1,
        total,
        bytes: (phase == Phase::Uploading).then_some(ByteCount {
            done: 23_000_000,
            total: 60_000_000,
        }),
    }
}

fn fill(run: u64, path: &str, status: FillStatus) -> FillRun {
    FillRun {
        run,
        folder: folder(path),
        status,
        total: 3,
        done: 1,
        declined: Vec::new(),
        short: None,
        unheld: Vec::new(),
    }
}

fn freeze(run: u64, path: &str, status: FreezeStatus) -> FreezeRun {
    FreezeRun {
        run,
        folder: folder(path),
        only: None,
        status,
        packs: 0,
        entries: 0,
        findings: Vec::new(),
        step: None,
    }
}

fn sync(run: u64, status: SyncStatus) -> SyncRun {
    SyncRun {
        run,
        status,
        added: 0,
        findings: Vec::new(),
        step: None,
    }
}

fn answer(
    library: &'static str,
    catalog: Standing,
    fill: Option<Latest<FillRun>>,
    sync: Option<SyncRun>,
    freeze: Option<Latest<FreezeRun>>,
) -> WorkDto {
    // The turn is the first of the two runs still going, as it is on a server
    // that took them up in that order; the other waits for it.
    let turn = if sync
        .as_ref()
        .is_some_and(|run| run.status == SyncStatus::Syncing)
    {
        Some(TurnTaker::Sync)
    } else if freeze
        .as_ref()
        .is_some_and(|latest| latest.on_record.status == FreezeStatus::Freezing)
    {
        Some(TurnTaker::Freeze)
    } else {
        None
    };
    WorkDto {
        server: SERVER.to_owned(),
        library,
        catalog: CatalogDto::of(&catalog),
        // Counted only while the Library is open, and nothing kept until a
        // case says otherwise.
        kept_parcels: (library == "unlocked").then(|| KeptParcelsDto::of(KeptTally::default())),
        turn: turn.map(TurnTaker::as_str),
        fill: fill.as_ref().map(FillDto::of),
        sync: sync.as_ref().map(|run| SyncDto::of(run, turn)),
        freeze: freeze.as_ref().map(|latest| FreezeDto::of(latest, turn)),
        delete: None,
        reconnect: None,
        log: Vec::new(),
    }
}

/// The same answer, with a deletion standing where `run` says and `waiting`
/// more behind it, holding the turn where it is running: every answer it is
/// put beside has nothing else running.
fn deleting(answer: WorkDto, run: DeleteRun, waiting: usize) -> WorkDto {
    let turn = (run.status == DeleteStatus::Deleting).then_some(TurnTaker::Delete);
    WorkDto {
        turn: turn.map(TurnTaker::as_str),
        delete: Some(DeleteDto::of(
            &DeleteReport {
                on_record: run,
                waiting,
            },
            turn,
        )),
        ..answer
    }
}

/// A deletion of `albums`, or of two files where `folder` is false.
fn deletion(run: u64, folder: bool, status: DeleteStatus) -> DeleteRun {
    let target = if folder {
        Target {
            folders: BTreeSet::from([entry_path("albums")]),
            paths: Default::default(),
        }
    } else {
        Target {
            folders: Default::default(),
            paths: [
                entry_path("books/vol-1/page-002.jpg"),
                entry_path("albums/a.jpg"),
            ]
            .into_iter()
            .collect(),
        }
    };
    DeleteRun {
        run,
        target,
        status,
        entries: 0,
        bytes: 0,
        removed: 0,
        rebuilt: 0,
        rebuild_read: 0,
        rebuild_written: 0,
        refused: Vec::new(),
        missing: Vec::new(),
        findings: Vec::new(),
        step: None,
    }
}

/// A commit the Library moved underneath, as a deletion's `stopped` carries it.
fn conflict() -> Reported {
    Reported::of(&ApiError::from(coffret_device::Error::Delete {
        cause: Box::new(coffret_device::DeleteError::Commit(
            coffret_device::CommitError::RemovalNotCurrent {
                container_ids: vec![ContainerId::from_bytes([3; ContainerId::BYTE_LEN])],
            }
            .into(),
        )),
    }))
}

/// The same answer, with a reconnect standing where `reconnect` says.
fn reconnecting(answer: WorkDto, reconnect: &Reconnect) -> WorkDto {
    WorkDto {
        reconnect: Some(ReconnectDto::of(reconnect)),
        ..answer
    }
}

/// An entry of every kind the log keeps, newest first, logged a minute apart.
fn logged() -> Vec<Logged> {
    let ended = vec![
        Ended::Fetch {
            path: "albums/cover.png".to_owned(),
            refusal: storage(),
        },
        Ended::Discarded {
            queue: Queue::Freeze,
            folder: folder("books/vol-2"),
        },
        Ended::Discarded {
            queue: Queue::Fill,
            folder: folder("books"),
        },
        Ended::Delete(DeleteRun {
            entries: 5,
            bytes: 8_200_000,
            removed: 5,
            ..deletion(1, true, DeleteStatus::Done)
        }),
        Ended::Freeze(FreezeRun {
            packs: 1,
            entries: 188,
            ..freeze(1, "books/vol-1", FreezeStatus::Done)
        }),
        Ended::Sync(sync(2, SyncStatus::Stopped(storage()))),
        Ended::Sync(SyncRun {
            added: 3,
            ..sync(1, SyncStatus::Done)
        }),
        Ended::Fill(fill(2, "albums", FillStatus::Done)),
        Ended::Fill(fill(1, "albums", FillStatus::Stopped(storage()))),
    ];
    let count = ended.len() as u64;
    ended
        .into_iter()
        .enumerate()
        .map(|(newer, ended)| {
            let id = count - newer as u64;
            Logged {
                id,
                at: SystemTime::UNIX_EPOCH + Duration::from_secs(1_793_000_000 + 60 * id),
                // The fill that stopped, which the one after it settled.
                settled: id == 1,
                ended,
            }
        })
        .collect()
}

fn alone<A>(run: A) -> Latest<A> {
    Latest {
        on_record: run,
        displaced: Vec::new(),
        waiting: Vec::new(),
        discarded: Vec::new(),
    }
}

/// Every state the answer can be in, each at least once.
fn every_answer() -> Vec<WorkDto> {
    // Nothing has run, and the Library is shut: the page that comes up to a
    // server started without its Passphrase.
    let idle = answer("locked", Standing::CaughtUp, None, None, None);

    // Everything under way at once, with a catch-up among it, and every phase a
    // run can say it is in.
    let running = answer(
        "unlocked",
        Standing::CatchingUp,
        Some(Latest {
            waiting: vec![folder("books")],
            ..alone(fill(2, "albums", FillStatus::Filling))
        }),
        Some(SyncRun {
            step: Some(step(Phase::Uploading, Some(4))),
            ..sync(1, SyncStatus::Syncing)
        }),
        Some(Latest {
            waiting: vec![folder("books/vol-2")],
            ..alone(FreezeRun {
                step: Some(step(Phase::Packing, None)),
                ..freeze(1, "books/vol-1", FreezeStatus::Freezing)
            })
        }),
    );
    let phases = [
        Phase::CatchingUp,
        Phase::Settling,
        Phase::Scanning,
        Phase::Fetching,
    ]
    .into_iter()
    .map(|phase| {
        answer(
            "unlocked",
            Standing::CaughtUp,
            None,
            Some(SyncRun {
                step: Some(step(phase, Some(2))),
                ..sync(1, SyncStatus::Syncing)
            }),
            None,
        )
    });

    // Every run finished, with every finding a run can report and an Entry a
    // fill declined for each reason a declined Entry carries.
    let declined = [
        DeviceSurfaced::ForeignFile {
            path: entry_path("albums/a.jpg"),
        },
        DeviceSurfaced::KeyLost {
            path: entry_path("albums/b.jpg"),
            container_id: ContainerId::from_bytes([7; ContainerId::BYTE_LEN]),
        },
    ]
    .iter()
    .map(|surfaced| Declined {
        path: surfaced.path().as_str().to_owned(),
        refusal: Reported::of(&ApiError::declined(surfaced)),
    })
    .collect::<Vec<_>>();
    let finished = answer(
        "unlocked",
        Standing::CaughtUp,
        Some(alone(FillRun {
            declined: declined.clone(),
            short: Some(ShortKeyring::new(Generation::FIRST, 3, 1, 0)),
            unheld: vec![UnheldParcel {
                container_id: ContainerId::from_bytes([8; ContainerId::BYTE_LEN]),
                index: 2,
                reason: UnheldReason::Missing,
            }],
            ..fill(3, "albums", FillStatus::Done)
        })),
        Some(SyncRun {
            added: 2,
            findings: every_finding(),
            ..sync(2, SyncStatus::Done)
        }),
        Some(alone(FreezeRun {
            packs: 1,
            entries: 12,
            findings: every_finding(),
            ..freeze(2, "books/vol-1", FreezeStatus::Done)
        })),
    );

    // A reader's fetches left parcels behind, which the Activity panel counts.
    let finished = WorkDto {
        kept_parcels: Some(KeptParcelsDto::of(KeptTally {
            count: 3,
            bytes: 96_000_000,
        })),
        ..finished
    };

    // Every run stopped, a catalog left behind, and what the queues lost and
    // the runs a later one took the record from beside them.
    let stopped = answer(
        "unlocked",
        Standing::Behind(Reported::gave_up()),
        Some(Latest {
            on_record: fill(5, "letters", FillStatus::Stopped(storage())),
            // One that stopped part way through a folder it had already
            // declined Entries of, so the rows those Entries are drawn in are
            // read off a displaced run too.
            displaced: vec![Displaced {
                run: FillRun {
                    declined,
                    short: Some(ShortKeyring::new(Generation::FIRST, 3, 0, 1)),
                    ..fill(4, "albums", FillStatus::Stopped(storage()))
                },
                stopped: storage(),
            }],
            waiting: Vec::new(),
            discarded: vec![folder("books")],
        }),
        // A sync whose commit failed after it had repaired the Keyring says the
        // repair on the run that stopped (spec: KL-15).
        Some(SyncRun {
            findings: vec![repaired_after_all()],
            ..sync(3, SyncStatus::Stopped(storage()))
        }),
        Some(Latest {
            on_record: freeze(
                4,
                "books/vol-2",
                FreezeStatus::Stopped(Reported::unfinished()),
            ),
            displaced: vec![Displaced {
                run: freeze(3, "books/vol-1", FreezeStatus::Stopped(storage())),
                stopped: storage(),
            }],
            waiting: Vec::new(),
            discarded: vec![folder("books/vol-3")],
        }),
    );

    // And the one state a fill has that nothing else does: followed away from.
    let superseded = answer(
        "unlocked",
        Standing::CaughtUp,
        Some(alone(fill(6, "", FillStatus::Superseded))),
        None,
        None,
    );

    // A catch-up the grant running out refused, with each state a reconnect
    // from it can stand in.
    let reconnects = [
        Reconnect::Waiting {
            url: "https://consent.example/".to_owned(),
        },
        Reconnect::Renewed,
        Reconnect::Refused,
        Reconnect::TimedOut,
        Reconnect::Failed,
    ]
    .into_iter()
    .map(|reconnect| {
        reconnecting(
            answer("unlocked", Standing::Behind(storage()), None, None, None),
            &reconnect,
        )
    });

    // The same book once it is stored, while the batch that adds it is
    // committed: the Keyring's replicas and then the head, one by one.
    let committing = answer(
        "unlocked",
        Standing::CaughtUp,
        None,
        None,
        Some(alone(FreezeRun {
            step: Some(step(Phase::Committing, Some(4))),
            ..freeze(1, "books/vol-1", FreezeStatus::Freezing)
        })),
    );

    // A book on its way to Storage: the one phase whose step counts bytes, on
    // the line that shows them.
    let sending = answer(
        "unlocked",
        Standing::CaughtUp,
        None,
        None,
        Some(alone(FreezeRun {
            step: Some(step(Phase::Uploading, Some(1))),
            ..freeze(1, "books/vol-1", FreezeStatus::Freezing)
        })),
    );

    // A deletion rebuilding a Pack, with another waiting; one that finished
    // with every kind of refusal and a file the Library did not hold; and one
    // the Library moved underneath.
    let quiet = || answer("unlocked", Standing::CaughtUp, None, None, None);
    let deletions = [
        deleting(
            quiet(),
            DeleteRun {
                step: Some(step(Phase::Packing, Some(2))),
                ..deletion(1, true, DeleteStatus::Deleting)
            },
            1,
        ),
        deleting(
            quiet(),
            DeleteRun {
                entries: 12,
                bytes: 340_000_000,
                removed: 10,
                rebuilt: 2,
                rebuild_read: 1_800_000_000,
                rebuild_written: 1_500_000_000,
                refused: ["key_lost", "unverified"]
                    .into_iter()
                    .map(|reason| RefusedPack {
                        spared: vec![entry_path("books/vol-1/page-002.jpg")],
                        kept: 2,
                        reason,
                    })
                    .collect(),
                missing: vec![entry_path("albums/gone.jpg")],
                findings: vec![repaired_after_all()],
                ..deletion(2, false, DeleteStatus::Done)
            },
            0,
        ),
        deleting(
            quiet(),
            deletion(3, true, DeleteStatus::Stopped(conflict())),
            0,
        ),
        // A deletion whose commit landed, removing this device's own copies
        // of what it took out of the Library: a phase that cannot count.
        deleting(
            quiet(),
            DeleteRun {
                step: Some(step(Phase::RemovingCopies, None)),
                ..deletion(4, true, DeleteStatus::Deleting)
            },
            0,
        ),
    ];

    // What the server has finished, an entry of every kind, newest first: a
    // failure standing and one a later run settled among them.
    let logged = WorkDto {
        log: logged().iter().map(super::LoggedDto::of).collect(),
        ..quiet()
    };

    let mut every = vec![idle, running, sending, committing, logged];
    every.extend(deletions);
    every.extend(phases);
    every.extend([finished, stopped, superseded]);
    every.extend(reconnects);
    every
}

// The explorer's half reads this file through its `FillRun` type and every
// union inside it; this is what holds the file to the server. A status, a
// phase or a field that changes on this side fails here until the file follows.
#[test]
fn the_work_the_explorer_reads_is_the_one_this_server_sends() {
    let written = serde_json::to_value(every_answer()).expect("an answer serializes");
    held_to(WORK, &written);
}
