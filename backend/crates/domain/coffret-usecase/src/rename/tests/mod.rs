//! The rename's contract, as tests (spec: FM-15, EP-5, EP-7, EP-10, CP-1).
//!
//! Two devices share one Library: the source device puts files into it from its
//! mapped folder and is the one that renames, and the target device fetches
//! into a folder of its own. What a case asserts about Storage is read off a
//! store that wrote down every request the run made, because the claim a
//! rename makes — no Container uploaded, none read — leaves nothing behind in
//! the Library to look at afterwards.

use std::path::Path;

use coffret_model::{EntryPath, EntryRename, Generation};

use crate::commit::CommitError;
use crate::entry_paths::entry_path;
use crate::fetch::fetch_folders;
use crate::fetch_conformance::fixtures::{
    at, entry_at, exists, freeze_source, keys, map, policy, read, request as fetch_request,
    sync_source, write,
};
use crate::fetch_conformance::{create_library, FetchUnderTest};
use crate::freeze::LibraryKeys;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::relocate::Relocation;
use crate::rename::{rename_entries, RenameError, RenameRefusal, RenameRequest, RenamedPath};
use crate::sync_conformance::fixtures::{touch, NEWER};
use crate::{InMemoryIndex, InMemoryStore};

mod recording_store;
use recording_store::RecordingStore;

mod detected;
mod pairs;
mod races;
mod refusals;
mod renamed;

/// A Pack target no case here comes near, so every frozen file lands in one
/// Pack (spec: PK-5).
const ONE_PACK: u64 = 16 * 1024 * 1024;

/// A Library with its first head, the source device mapping its folder onto
/// the Library root and the target device mapping nothing yet.
async fn fixture() -> FetchUnderTest {
    let fixture = FetchUnderTest::new(
        Box::new(InMemoryStore::new(3)),
        Box::new(InMemoryIndex::new()),
        Box::new(InMemoryIndex::new()),
    );
    create_library(&fixture).await;
    map(
        fixture.source(),
        fixture.fs(),
        None,
        fixture.source_folder(),
    )
    .await;
    fixture
}

/// The clock every rename here runs at: after any run a case makes before it,
/// and none of them reads it back.
fn renamed_at() -> crate::device_state::DeviceTime {
    at(100)
}

/// The request renaming `from` to `to` through the device whose catalog is
/// `index`, against `store`, relocating that device's files after the commit.
fn request<'a>(
    store: &'a dyn ObjectStore,
    index: &'a dyn Index,
    fixture: &'a FetchUnderTest,
    keys: &'a LibraryKeys,
    from: &str,
    to: &str,
) -> RenameRequest<'a> {
    RenameRequest::new(
        store,
        index,
        keys,
        fixture.fs(),
        entry_path(from),
        entry_path(to),
        renamed_at(),
    )
    .with_policy(policy())
    .with_local_disk(fixture.fs(), fixture.fs(), fixture.fs())
}

/// The head a catalog stands at.
async fn head(index: &dyn Index) -> Generation {
    index
        .checkpoint()
        .await
        .expect("reading the checkpoint must succeed")
        .expect("the catalog stands at a head")
        .head_generation()
}

/// Whether the catalog holds a current Entry at `path`.
async fn holds(index: &dyn Index, path: &str) -> bool {
    index
        .entry_at(&entry_path(path))
        .await
        .expect("asking the catalog must succeed")
        .is_some()
}

/// The file at `relative` under `folder`.
fn file(fixture: &FetchUnderTest, folder: &Path, relative: &str) -> Option<Vec<u8>> {
    let path = folder.join(relative);
    exists(fixture.fs(), &path).then(|| read(fixture.fs(), &path))
}

/// One rename the outcome reports.
fn renamed(from: &str, to: &str) -> RenamedPath {
    RenamedPath {
        from: entry_path(from),
        to: entry_path(to),
    }
}

/// The paths a record's renames give, in the order it holds them.
fn record_paths(renames: &[EntryRename]) -> Vec<&str> {
    renames
        .iter()
        .map(|rename| rename.path().as_str())
        .collect()
}

/// The files the relocations relocated, as `(from, to)`.
fn relocated(relocations: &[Relocation]) -> Vec<(EntryPath, EntryPath)> {
    let mut relocated: Vec<_> = relocations
        .iter()
        .filter_map(|relocation| match relocation {
            Relocation::Relocated { from, to } => Some((from.clone(), to.clone())),
            _ => None,
        })
        .collect();
    relocated.sort();
    relocated
}
