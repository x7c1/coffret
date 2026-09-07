use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::device_state::RootIdentity;
use crate::in_memory_fs::state::faults::Fault;

// What the fake has been told to refuse, and the counting that finds the
// invocation a case named.
mod faults;

// What happens to one file: the calls a spool and a placement make on it, and
// what a case plants before either runs.
mod files;

// And to the tree the files stand in: the folders a run makes on its way down,
// and the two walks that reach one.
mod folders;

// What a case, or a capability answering one, reads back off the fake without
// changing it.
mod inspecting;

// What the fake says the filesystem under a mapped root is (spec: EP-12).
mod roots;

/// The fake's state, taken even from a lock a panicking case poisoned: what is
/// behind it is a case's own bookkeeping, and a poisoned lock would replace the
/// failure that panicked with one about the lock.
///
/// One function for the whole fake rather than one per handle: the filesystem
/// itself and everything it hands out — a writer, a reader, a destination, a
/// scratch file, a flushed file — hold the same `Mutex<State>` and take it for
/// the same reason.
pub(in crate::in_memory_fs) fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What a file the fake holds is, beyond its bytes.
///
/// The times are here rather than derived from a clock, because what every case
/// about them is asking is whether a *particular* time reached a record: a fake
/// that stamped "now" would make the assertion depend on when the case ran.
#[derive(Debug)]
struct FileNode {
    bytes: Vec<u8>,
    mtime: i64,
    btime: Option<i64>,
}

/// What the fake holds, and what it has been told to refuse.
///
/// Directories are kept apart from files for the one reason a spool needs them:
/// a real spool file cannot be created under a directory nobody made, and a fake
/// that let it be would answer a case about
/// [`prepare_dir`](crate::Spool::prepare_dir) with a success the device would
/// never give. The mapped-folder side needs them for a second reason — a folder
/// is something a listing reports and a walk descends into — so one set of
/// directories serves both.
///
/// `others` is what the fake has instead of a symbolic link: a name that is
/// neither a file nor a folder, so a case can arrange EP-8's rule without a real
/// filesystem to make a link on.
///
/// The calls against all of it are grouped a module apiece rather than gathered
/// into one impl block, because every capability the fake gains is more of them:
/// the three it answers today take more than two dozen, and a fourth would be
/// added to whichever of these groups it belonged in.
#[derive(Debug, Default)]
pub(super) struct State {
    dirs: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, FileNode>,
    others: BTreeSet<PathBuf>,
    identities: BTreeMap<PathBuf, RootIdentity>,
    script: Vec<Fault>,
    attempts: BTreeMap<u8, usize>,
}

/// The modification time a file the fake was handed carries until a case says
/// otherwise.
///
/// Fixed rather than taken from a clock, so that what a case writes into a
/// device's bookkeeping is the same on every machine and on every run.
const DEFAULT_MTIME: i64 = 1_700_000_000;

/// A file with no bytes and the fake's default stamps.
fn fresh() -> FileNode {
    FileNode {
        bytes: Vec::new(),
        mtime: DEFAULT_MTIME,
        btime: Some(DEFAULT_MTIME),
    }
}

/// Whether `path` is `dir` itself or something beneath it.
///
/// Component-wise, which is what [`Path::starts_with`] is: a folder called
/// `photographs-old` is not under one called `photographs`.
fn under(path: &Path, dir: &Path) -> bool {
    path.starts_with(dir)
}
