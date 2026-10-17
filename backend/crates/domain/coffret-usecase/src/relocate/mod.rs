//! Relocating the files this device placed to follow renames of their Entries
//! (spec: EP-10).
//!
//! A rename changes an Entry's current path and leaves this device's file of
//! it where it was placed, its record waiting to follow the Entry
//! ([`LocalEntry::relocate_to`]). Two runs relocate every such file before
//! they scan or place anything: a [`sync`](crate::sync), which holds every
//! mapped root for its scan and its discards, and a [`fetch`](crate::fetch),
//! which holds them for its placements. Relocating first is what lets the rest
//! of either run read the folder the way the Library names it: the scan then
//! finds the file at its Entry's own path, and a fetch finds the place it
//! would have filled already taken by this device's own copy.
//!
//! For each waiting file the run asks, in this order, and stops at the first
//! answer that refuses (each refusal is a [`Relocation::Refused`], the file and
//! its record left exactly as they were, and asked about again by the next
//! run):
//!
//! - whether both paths lie under one mapped root, and the new one through no
//!   name a mapped folder keeps for itself;
//! - whether that root is there to be read from (spec: EP-12) — a root whose
//!   filesystem moved is not taken for one, whatever it holds: a scan that
//!   finds it holding files re-stamps it, and the next run relocates then;
//! - whether a symbolic link stands on the way to either path (spec: EP-8);
//! - whether the file is still what this device placed: its length and
//!   modification time as recorded, or its content the recorded hash
//!   (spec: EP-15);
//! - and what stands at the new path. Nothing is the ordinary case. This
//!   device's own unedited copy of the same Entry — a fetch placed it there
//!   while this file waited — makes this file redundant, and it is discarded
//!   into the desktop's trash and its record forgotten. Another waiting file of
//!   this device's, about to be relocated itself, is waited for, and two or more
//!   waiting files that stand in each other's places are exchanged. Anything
//!   else is a file this device did not place there, and nothing overwrites it.
//!
//! The file is relocated first and the record follows it: one rename on disk, then
//! the record re-keyed to the new path. An interruption between the two leaves
//! the old record and the file at the new path, which the next run recognizes
//! — the file is gone from where the record says and stands where its Entry
//! does, as the record describes it — and finishes by re-keying, with nothing
//! relocated twice and nothing overwritten. An interrupted exchange is recognized
//! the same way, by each file standing where the other's record says. A
//! record never points at a path its file was not relocated to.
//!
//! A file that is gone from where it stood has nothing to relocate. Its record
//! follows the Entry to its new path where nothing stands there, so the scan
//! reports the Entry as deleted locally, as it would have at the old path
//! (spec: EP-10); where something does stand there, the record cannot describe
//! it and is forgotten. An absent record — a deletion this device witnessed —
//! follows its Entry the same way.
//!
//! The folders the relocations left empty are removed as the discards' are
//! (spec: EP-15). Nothing here uploads, encodes, or commits anything: a
//! relocation is this device catching its own disk up with the Library.

use std::collections::BTreeMap;
use std::sync::Arc;

use coffret_model::EntryPath;

use crate::departure::EmptiedFolders;
use crate::descent_error::DescentError;
use crate::device_state::{LocalEntry, Mapping};
use crate::index::Index;
use crate::index_error::IndexResult;
use crate::local_scan::SourceFile;
use crate::local_trash::LocalTrash;
use crate::mapped_roots::MappedRoots;
use crate::relocate_error::RelocateError;
use crate::relocations::Relocations;

mod relocation;
pub use relocation::Relocation;

mod relocation_refused;
pub use relocation_refused::RelocationRefused;

// What stands on disk for each waiting record, and what refuses it outright.
mod look;
use look::{Look, Looked};

// A rename on disk an interrupted run left without its record.
mod finish_interrupted;

// Refused, discarded, gone, or to relocate: each record's answer.
mod decide;
use decide::Decided;

// The relocations whose destination is free, one rename each.
mod relocate_ready;

// Files standing in each other's places, exchanged.
mod exchange_cycle;
use exchange_cycle::cycle_in;

mod discard;

mod follow_gone;

/// The three capabilities over this device's disk a relocation is made
/// through: the mapped folders as a walk reads them, the rename, and the trash
/// a redundant copy goes to.
#[derive(Clone, Copy)]
pub(crate) struct LocalDisk<'a> {
    pub(crate) roots: &'a dyn MappedRoots,
    pub(crate) relocations: &'a dyn Relocations,
    pub(crate) trash: &'a dyn LocalTrash,
}

/// Relocates every file of this device's that waits to follow its Entry, and
/// says what became of each (spec: EP-10).
///
/// Only a failure of the Index is an error: everything the disk refuses is a
/// finding, and the rest of the run goes on.
pub(crate) async fn relocate_waiting(
    index: &dyn Index,
    disk: LocalDisk<'_>,
) -> IndexResult<Vec<Relocation>> {
    let waiting = index.awaiting_relocation().await?;
    if waiting.is_empty() {
        return Ok(Vec::new());
    }
    let mut run = Run {
        index,
        disk,
        mappings: index.mappings().await?,
        roots: BTreeMap::new(),
        found: Vec::new(),
        emptied: EmptiedFolders::default(),
    };

    // What an interrupted run left half done is finished before anything is
    // planned against the records it left, and the records are read again.
    let mut looked = run.look_all(waiting).await;
    if run.finish_interrupted(&looked).await? {
        let waiting = index.awaiting_relocation().await?;
        looked = run.look_all(waiting).await;
    }
    run.relocate(looked).await?;

    let Run { found, emptied, .. } = run;
    // Once every relocation is made, so a folder several files left is asked about
    // after the last of them did.
    emptied.remove(disk.trash).await;
    Ok(found)
}

/// One run's relocations, and what it has found out on the way.
struct Run<'a> {
    index: &'a dyn Index,
    disk: LocalDisk<'a>,
    mappings: Vec<Mapping>,
    /// Whether each mapping's root may be relocated in, asked once per
    /// mapping.
    roots: BTreeMap<Option<EntryPath>, Option<RelocationRefused>>,
    found: Vec<Relocation>,
    emptied: EmptiedFolders,
}

/// A file to relocate, its source found and vouched for.
struct Pending {
    looked: Looked,
    source: SourceFile,
}

impl Run<'_> {
    /// Decides each record, then relocates the files, the ones whose destination
    /// is free first and the ones that stand in each other's places last.
    async fn relocate(&mut self, looked: Vec<Look>) -> IndexResult<()> {
        let Decided { mut pending, gone } = self.decide(looked).await?;

        self.relocate_ready(&mut pending).await?;
        while let Some(cycle) = cycle_in(&pending) {
            let members: Vec<Pending> = cycle
                .iter()
                .map(|from| {
                    pending
                        .remove(from)
                        .expect("a cycle is made of pending relocations")
                })
                .collect();
            self.exchange_cycle(members).await?;
            // A relocation that waited on a member of the cycle can be made now, or
            // is refused by what the cycle left at its destination.
            self.relocate_ready(&mut pending).await?;
        }

        for one in gone {
            self.follow_gone(one).await?;
        }
        Ok(())
    }

    /// Whether `file` is the one `local` describes: its length and
    /// modification time as recorded, and its content the recorded hash where
    /// there is one. A file that cannot be read is not taken for it.
    async fn holds_copy(&self, local: &LocalEntry, file: &SourceFile) -> bool {
        if !agrees(local, file) {
            return false;
        }
        match local.observation.hash {
            None => true,
            Some(recorded) => file
                .content_hash(self.disk.roots)
                .await
                .is_ok_and(|hash| hash == recorded),
        }
    }

    fn refuse(&mut self, local: &LocalEntry, reason: RelocationRefused) {
        self.found.push(Relocation::Refused {
            from: local.observation.path.clone(),
            to: local
                .relocate_to
                .clone()
                .expect("only a waiting record is relocated"),
            reason,
        });
    }
}

/// Whether the file on disk is still what the record describes, by the cheap
/// comparison alone: the length and modification time a rename leaves as they
/// were (spec: EP-11).
fn agrees(local: &LocalEntry, file: &SourceFile) -> bool {
    local.observation.size == file.size && local.observation.mtime == file.mtime
}

/// What a refused rename on disk means for the file.
fn relocate_refusal(refused: RelocateError) -> RelocationRefused {
    match refused {
        RelocateError::Taken { .. } => RelocationRefused::DestinationTaken,
        RelocateError::AnotherFilesystem { .. } => RelocationRefused::AnotherFilesystem,
        RelocateError::NoAtomicRename { cause, .. } => {
            RelocationRefused::NoAtomicRename(Arc::new(cause))
        }
        RelocateError::Descent(DescentError::Blocked { stopped_at }) => {
            RelocationRefused::SymbolicLink { stopped_at }
        }
        RelocateError::Descent(refused) => disk(refused),
    }
}

fn disk(refused: DescentError) -> RelocationRefused {
    RelocationRefused::Disk(Arc::new(refused))
}
