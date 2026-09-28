use coffret_device::DegradedKeyring;

use crate::finding::Finding;
use crate::folder::Folder;

use super::{Declined, FillStatus};

/// What one fill of one folder has come to.
///
/// The server's own state and not the Library's: it is about work in flight on
/// this device, it is gone when the process is, and none of it is ever
/// uploaded. The catalog keeps saying `present` or `remote` about every Entry
/// throughout (spec: EP-10) — this says which of the `remote` ones something is
/// doing something about right now.
#[derive(Clone, Debug)]
pub struct FillRun {
    /// Which run of the fill this is, counted from the start of this process.
    ///
    /// What a screen tells one run's account of itself from the next's: a line
    /// somebody has read and put away must not take the next run's line with it,
    /// and nothing else here distinguishes two runs that came to the same thing.
    /// It is stamped on by [`Fills`](super::Fills) rather than carried here from
    /// the flow — a run is a folder taken off the queue, and what counts them is
    /// the one thing that sees them all — so a fresh one is `0` until it is
    /// published.
    pub run: u64,
    /// The folder being brought over.
    pub folder: Folder,
    /// Where the fill stands, and what stopped it where something did.
    ///
    /// One refusal and not one per Entry: what stops a fill is Storage being
    /// unreachable or the grant having run out, and every Entry left in the
    /// folder would have met it identically.
    pub status: FillStatus,
    /// How many of the folder's files the fill set out to bring over — the
    /// `remote` rows of the listing it started from, and `0` until it has read
    /// that listing.
    pub total: usize,
    /// How many of them are on this device now.
    pub done: usize,
    /// The Entries the fill did not bring over, and what it found instead.
    pub declined: Vec<Declined>,
    /// What the fill's reads found of the committed Keyring, where one of them
    /// had to step over a position of the set (spec: KL-5, KL-15) — and `None`
    /// where none did.
    ///
    /// One for the whole run however many Entries met it, as a flow of the
    /// device's own says it once: each Entry's fetch reads the set afresh, and a
    /// line per Entry would be one fact said as many times as the folder has
    /// files. Where the reads disagree, `graver` says which one is kept.
    pub degraded: Option<DegradedKeyring>,
}

impl FillRun {
    /// A fill that has been armed and has not read its folder's listing yet.
    pub(super) fn starting(folder: Folder) -> Self {
        Self {
            run: 0,
            folder,
            status: FillStatus::Filling,
            total: 0,
            done: 0,
            declined: Vec::new(),
            degraded: None,
        }
    }

    /// Takes in what one read found of the committed Keyring.
    pub(super) fn read_keyring(&mut self, found: DegradedKeyring) {
        self.degraded = Some(graver(self.degraded, found));
    }

    /// What the run found and did not act on, as the browser is told it.
    ///
    /// None of it stops the run or asks anything of whoever reads it: the
    /// files still open (spec: RV-2), and the next run that writes to the
    /// Library examines the set and repairs what it finds lost before it
    /// commits (spec: KL-13, KL-16).
    pub fn findings(&self) -> Vec<Finding> {
        self.degraded
            .iter()
            .map(coffret_device::Finding::from)
            .filter_map(|found| Finding::of(&found))
            .collect()
    }
}

/// The one of two reports about the committed Keyring that says the most.
///
/// A position established as lost outranks a replica Storage merely did not
/// hand over, which the next read may fetch perfectly well (spec: KL-15);
/// between two of the same weight the first one stands, so a report does not
/// change under a reader for nothing.
pub(super) fn graver(kept: Option<DegradedKeyring>, found: DegradedKeyring) -> DegradedKeyring {
    match kept {
        Some(kept) if kept.established() || !found.established() => kept,
        _ => found,
    }
}
