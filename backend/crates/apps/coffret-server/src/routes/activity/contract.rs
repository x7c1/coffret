//! Every state the activity answer can be in, written out as the wire carries
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

use std::path::PathBuf;

use coffret_device::{
    FindingReason, Phase, RootRefused, RootUnavailable, Step, Surfaced as DeviceSurfaced,
};
use coffret_model::ContainerId;

use super::{ActivityDto, CatalogDto, FillDto, FreezeDto, SyncDto};
use crate::api_error::{held_to, ApiError};
use crate::entry_paths::entry_path;
use crate::fill::{Activity, Declined, FillStatus};
use crate::finding::Finding;
use crate::folder::Folder;
use crate::freeze::{FreezeActivity, FreezeStatus};
use crate::latest::Latest;
use crate::refresh::Standing;
use crate::reported::Reported;
use crate::sync::{SyncActivity, SyncStatus};

/// Where the explorer reads the activity answers from, relative to this crate.
const ACTIVITY: &str = "../../../../frontend/packages/gateway/api/src/contract/activity.json";

/// What every answer here calls the process that gave it.
///
/// A constant rather than one drawn: the real name is entropy, and a file that
/// changed on every run could hold nothing to anything.
const SERVER: &str = "contract-server";

fn folder(path: &str) -> Folder {
    Folder::named((!path.is_empty()).then(|| entry_path(path)))
}

/// A refusal as a run's `stopped` carries it.
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
        unavailable(RootUnavailable::Missing),
        unavailable(RootUnavailable::AnotherFilesystem),
        Found::RefusedRoot {
            prefix: None,
            local_root: PathBuf::from("/mnt/copied"),
            reason: RootRefused::MarkerMismatch,
        },
        Found::LockedContainer {
            container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
        },
    ]
    .iter()
    .map(|found| Finding::of(found).expect("every finding here is one a browser is told"))
    .collect()
}

/// A step in `phase`, counted or not.
fn step(phase: Phase, total: Option<usize>) -> Step {
    Step {
        phase,
        done: 1,
        total,
    }
}

fn fill(run: u64, path: &str, status: FillStatus) -> Activity {
    Activity {
        run,
        folder: folder(path),
        status,
        total: 3,
        done: 1,
        declined: Vec::new(),
        stopped: None,
    }
}

fn freeze(run: u64, path: &str, status: FreezeStatus) -> FreezeActivity {
    FreezeActivity {
        run,
        folder: folder(path),
        status,
        packs: 0,
        entries: 0,
        findings: Vec::new(),
        step: None,
        stopped: None,
    }
}

fn sync(run: u64, status: SyncStatus) -> SyncActivity {
    SyncActivity {
        run,
        status,
        added: 0,
        findings: Vec::new(),
        step: None,
        stopped: None,
    }
}

fn answer(
    library: &'static str,
    catalog: Standing,
    fill: Option<Latest<Activity>>,
    sync: Option<SyncActivity>,
    freeze: Option<Latest<FreezeActivity>>,
) -> ActivityDto {
    ActivityDto {
        server: SERVER.to_owned(),
        library,
        catalog: CatalogDto::of(&catalog),
        fill: fill.as_ref().map(FillDto::of),
        sync: sync.as_ref().map(SyncDto::of),
        freeze: freeze.as_ref().map(FreezeDto::of),
    }
}

fn alone<A>(activity: A) -> Latest<A> {
    Latest {
        activity,
        displaced: Vec::new(),
        waiting: Vec::new(),
        dropped: Vec::new(),
    }
}

/// Every state the answer can be in, each at least once.
fn every_answer() -> Vec<ActivityDto> {
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
        Some(SyncActivity {
            step: Some(step(Phase::Uploading, Some(4))),
            ..sync(1, SyncStatus::Syncing)
        }),
        Some(Latest {
            waiting: vec![folder("books/vol-2")],
            ..alone(FreezeActivity {
                step: Some(step(Phase::Packing, None)),
                ..freeze(1, "books/vol-1", FreezeStatus::Freezing)
            })
        }),
    );
    let phases = [
        Phase::CatchingUp,
        Phase::Reconciling,
        Phase::Scanning,
        Phase::Fetching,
    ]
    .into_iter()
    .map(|phase| {
        answer(
            "unlocked",
            Standing::CaughtUp,
            None,
            Some(SyncActivity {
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
    .collect();
    let finished = answer(
        "unlocked",
        Standing::CaughtUp,
        Some(alone(Activity {
            declined,
            ..fill(3, "albums", FillStatus::Done)
        })),
        Some(SyncActivity {
            added: 2,
            findings: every_finding(),
            ..sync(2, SyncStatus::Done)
        }),
        Some(alone(FreezeActivity {
            packs: 1,
            entries: 12,
            findings: every_finding(),
            ..freeze(2, "books/vol-1", FreezeStatus::Done)
        })),
    );

    // Every run stopped, a catalog left behind, and what the queues lost and
    // the runs a later one took the record from beside them.
    let stopped = answer(
        "unlocked",
        Standing::Behind(Reported::gave_up()),
        Some(Latest {
            activity: Activity {
                stopped: Some(storage()),
                ..fill(5, "letters", FillStatus::Stopped)
            },
            displaced: vec![Activity {
                stopped: Some(storage()),
                ..fill(4, "albums", FillStatus::Stopped)
            }],
            waiting: Vec::new(),
            dropped: vec![folder("books")],
        }),
        Some(SyncActivity {
            stopped: Some(storage()),
            ..sync(3, SyncStatus::Stopped)
        }),
        Some(Latest {
            activity: FreezeActivity {
                stopped: Some(Reported::unfinished()),
                ..freeze(4, "books/vol-2", FreezeStatus::Stopped)
            },
            displaced: vec![FreezeActivity {
                stopped: Some(storage()),
                ..freeze(3, "books/vol-1", FreezeStatus::Stopped)
            }],
            waiting: Vec::new(),
            dropped: vec![folder("books/vol-3")],
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

    let mut every = vec![idle, running];
    every.extend(phases);
    every.extend([finished, stopped, superseded]);
    every
}

// The explorer's half reads this file through its `Activity` type and every
// union inside it; this is what holds the file to the server. A status, a
// phase or a field that changes on this side fails here until the file follows.
#[test]
fn the_activity_the_explorer_reads_is_the_one_this_server_sends() {
    let written = serde_json::to_value(every_answer()).expect("an answer serializes");
    held_to(ACTIVITY, &written);
}
