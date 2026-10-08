use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use coffret_model::{EntryPath, Mtime};

use crate::local_scan::source_file::SourceFile;

/// The files this device placed into a mapped folder without knowing when they
/// came into being.
///
/// A file dropped onto the explorer arrives as its name, its bytes and at most
/// its modification time: a browser does not say when the person's own file
/// was created. The file the drop writes has a birth time all the same — the
/// moment the drop wrote it — and on a filesystem that reports one, a scan
/// would read that moment off it and record it as the Entry's `original_btime`.
/// That is the one value FM-9 says is never filled in, and this one would not
/// even be a stand-in: it is a fact about the copy and not about the file.
///
/// So the drop says here what it placed, and the scan of the sync or freeze
/// that carries it into the Library records no birth time for it (spec: FM-9,
/// EP-11). Nothing else about the file changes, and a file put into a mapped
/// folder any other way is not here and keeps the birth time its filesystem
/// reports.
///
/// This is not a record of materialization: a file here is still merely
/// added, and a scan reports it only as new (spec: EP-10).
///
/// What it holds is the file as the drop left it — its length, and the
/// modification time it was stamped with where the drop was told one — so a
/// file that has since been replaced by one of another length or time is no
/// longer taken for it. A path whose file no longer matches is forgotten the
/// first time a scan finds it so.
///
/// It is held in memory and nowhere else, for as long as the open Library that
/// carries it: the scan it is for is the one the drop arms a moment later. A
/// file whose scan comes after this device forgot the drop — the Library was
/// locked or the process ended first — is read like any other file.
#[derive(Debug, Default)]
pub struct UnknownBirths {
    placed: Mutex<BTreeMap<EntryPath, Placed>>,
}

/// What a drop left at one Entry Path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placed {
    size: u64,
    /// The modification time the drop stamped, or `None` where it was told none
    /// and the file keeps the time it was written at — which nobody here read,
    /// so the length is then the whole of what is compared.
    mtime: Option<Mtime>,
}

impl UnknownBirths {
    /// Nothing placed yet.
    pub const fn new() -> Self {
        Self {
            placed: Mutex::new(BTreeMap::new()),
        }
    }

    /// Records that the file now at `path` was placed by this device with no
    /// birth time of its own, `size` bytes long and stamped with `mtime` where
    /// it was stamped at all.
    ///
    /// A second placement at the same path replaces the first, which is what the
    /// rename that published it did to the file.
    pub fn record(&self, path: EntryPath, size: u64, mtime: Option<Mtime>) {
        self.placed().insert(path, Placed { size, mtime });
    }

    /// Whether the file a scan found is one this device placed with no birth
    /// time of its own.
    ///
    /// A path that was placed and now holds a file of another length or time is
    /// forgotten, so that a later scan of a file the person put there does not
    /// meet the drop's record again.
    pub(crate) fn holds(&self, source: &SourceFile) -> bool {
        let mut placed = self.placed();
        let Some(dropped) = placed.get(&source.path) else {
            return false;
        };
        let same =
            dropped.size == source.size && dropped.mtime.is_none_or(|mtime| mtime == source.mtime);
        if !same {
            placed.remove(&source.path);
        }
        same
    }

    /// The map, whatever a thread that panicked while holding it left in it.
    ///
    /// Every write is one insertion or one removal, so there is no half-made
    /// state for a panic to have left behind.
    fn placed(&self) -> MutexGuard<'_, BTreeMap<EntryPath, Placed>> {
        self.placed.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The record a flow that was handed none reads: nothing was placed.
pub(crate) static NONE_PLACED: UnknownBirths = UnknownBirths::new();

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::entry_paths::entry_path;
    use crate::MappedRelativeLocation;

    fn found(path: &str, size: u64, mtime: i64) -> SourceFile {
        SourceFile {
            path: entry_path(path),
            root: PathBuf::from("/folder"),
            relative: MappedRelativeLocation::from_component(path.into()),
            size,
            mtime: Mtime::from_unix_seconds(mtime),
            btime: None,
        }
    }

    // FM-9, EP-11: what the drop placed is recognised by what it placed, and a
    // file that has since become another is read like any other — and stays
    // read that way, because the record is gone.
    #[test]
    fn a_placed_file_is_held_until_it_is_found_changed() {
        let births = UnknownBirths::new();
        births.record(entry_path("a.jpg"), 4, Some(Mtime::from_unix_seconds(-2)));
        births.record(entry_path("b.jpg"), 4, None);

        assert!(births.holds(&found("a.jpg", 4, -2)));
        assert!(
            births.holds(&found("a.jpg", 4, -2)),
            "asking twice changes nothing"
        );
        assert!(
            births.holds(&found("b.jpg", 4, 1_700_000_000)),
            "no stamp, so the length is the whole of what is compared",
        );
        assert!(!births.holds(&found("c.jpg", 4, -2)), "never placed");

        assert!(!births.holds(&found("a.jpg", 4, 1_700_000_000)));
        assert!(
            !births.holds(&found("a.jpg", 4, -2)),
            "a file found changed is forgotten, so its record does not come back",
        );
        assert!(!births.holds(&found("b.jpg", 5, 1_700_000_000)));
    }
}
