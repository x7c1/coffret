use coffret_device::{Findings, Relocation, RenameOutcome, RenamePair};

use crate::finding::Finding;
use crate::reported::Reported;

use super::MoveStatus;

/// What one move came to.
///
/// The server's own state and not the Library's, as every flow's account of a
/// run is: what the Library holds afterwards is the listing's to say.
#[derive(Clone, Debug)]
pub struct MoveRun {
    /// What it was asked to move, in the order the request named it.
    pub pairs: Vec<RenamePair>,
    /// How it ended.
    pub status: MoveStatus,
    /// How many Entries the record renamed, and `0` where it stopped.
    pub entries: usize,
    /// How many of this device's files followed their Entries (spec: EP-10).
    pub relocated: usize,
    /// How many of them could not follow and left this device, discarded
    /// into the trash (spec: EP-10).
    pub left: usize,
    /// What kept this device's files of the renamed Entries from following
    /// them, on a move whose record committed and whose relocations then
    /// failed: they stay where they were, and the next sync or fetch relocates
    /// them (spec: EP-10). `None` where they were relocated, and on a move that
    /// stopped.
    pub unrelocated: Option<Reported>,
    /// What became of this device's files of the renamed Entries, what the
    /// commit left for later and the Keyring repairs it performed
    /// (spec: EP-10, OC-6, KL-15) — on a move that stopped, the repairs alone.
    pub findings: Vec<Finding>,
}

impl MoveRun {
    /// A move of `pairs` that has done nothing yet.
    pub(crate) fn starting(pairs: Vec<RenamePair>) -> Self {
        Self {
            pairs,
            status: MoveStatus::Done,
            entries: 0,
            relocated: 0,
            left: 0,
            unrelocated: None,
            findings: Vec::new(),
        }
    }

    /// The move committed, with `outcome`.
    pub(super) fn done(&mut self, outcome: &RenameOutcome) {
        self.entries = outcome.renamed.len();
        self.relocated = outcome
            .relocations
            .iter()
            .filter(|relocation| matches!(relocation, Relocation::Relocated { .. }))
            .count();
        self.left = outcome
            .relocations
            .iter()
            .filter(|relocation| matches!(relocation, Relocation::Left { .. }))
            .count();
        self.findings = Finding::all_of(&Findings::from(outcome));
        self.status = MoveStatus::Done;
    }

    /// The move committed its record of `entries` renamed Entries, and this
    /// device's files of them were not relocated, for what `refusal` says.
    pub(super) fn unrelocated(&mut self, entries: usize, refusal: Reported) {
        self.entries = entries;
        self.unrelocated = Some(refusal);
        self.status = MoveStatus::Done;
    }

    /// The pairs, as the wire spells them.
    pub fn pairs_named(&self) -> Vec<(String, String)> {
        self.pairs
            .iter()
            .map(|pair| (pair.from.as_str().to_owned(), pair.to.as_str().to_owned()))
            .collect()
    }
}
