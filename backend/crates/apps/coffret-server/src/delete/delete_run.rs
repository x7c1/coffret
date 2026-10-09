use coffret_device::{DeleteOutcome, DeletePreview, EntryPath, Findings, PackRefusal, Step};

use crate::finding::Finding;

use super::{DeleteStatus, Target};

/// What one deletion has come to.
///
/// The server's own state and not the Library's: it is about work in flight on
/// this device, it is gone when the process is, and none of it is ever
/// uploaded. What the Library holds afterwards is the listing's to say.
///
/// The counts are outcomes and stay `0` until the batch commits: a deletion is
/// one batch (spec: PK-9, CP-1), so until it has committed no number of
/// Entries gone would be true. Where the run has got to is
/// [`step`](Self::step).
#[derive(Clone, Debug)]
pub struct DeleteRun {
    /// Which run of the deletion this is, counted from the start of this
    /// process. Stamped on by [`Deletes`](super::Deletes).
    pub run: u64,
    /// What it was asked to delete.
    pub target: Target,
    /// Where it stands, and what stopped it where something did.
    pub status: DeleteStatus,
    /// How many Entries left the Library.
    pub entries: usize,
    /// Their total length, in plaintext bytes (spec: FM-9).
    pub bytes: u64,
    /// How many Containers were removed outright (spec: PK-9).
    pub removed: usize,
    /// How many Packs were rebuilt around the Entries they keep (spec: PK-10).
    pub rebuilt: usize,
    /// How many bytes those rebuilds read from Storage.
    pub rebuild_read: u64,
    /// How many bytes the replacements they wrote weigh on Storage.
    pub rebuild_written: u64,
    /// The Packs the deletion was refused for, and the named files each kept.
    pub refused: Vec<RefusedPack>,
    /// Named files the Library held no current Entry at.
    pub missing: Vec<EntryPath>,
    /// What its commit left for later and the Keyring repairs it performed
    /// (spec: OC-6, KL-15) — on a run that stopped, the repairs alone.
    pub findings: Vec<Finding>,
    /// How far into the run the flow has got, and `None` before it has said
    /// and once it is over.
    pub step: Option<Step>,
}

impl DeleteRun {
    /// A deletion that has been armed and has done nothing yet.
    pub(super) fn starting(target: Target) -> Self {
        Self {
            run: 0,
            target,
            status: DeleteStatus::Deleting,
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

    /// The run finished with `outcome`.
    pub(super) fn done(&mut self, outcome: &DeleteOutcome) {
        self.entries = outcome.entries();
        self.bytes = outcome.bytes;
        self.removed = outcome.removed.len();
        self.rebuilt = outcome.rebuilt.len();
        self.rebuild_read = outcome.rebuild_read();
        self.rebuild_written = outcome.rebuild_written();
        self.refused = outcome.refused.iter().map(RefusedPack::of).collect();
        self.missing = outcome.missing.clone();
        self.findings = Finding::all_of(&Findings::from(outcome));
        self.status = DeleteStatus::Done;
    }
}

/// One Pack a deletion was refused for: left exactly as it was, with every
/// Entry it held — the named ones too — still in the Library (spec: PK-10,
/// KL-17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusedPack {
    /// The named files that stay because their Pack does.
    pub spared: Vec<EntryPath>,
    /// How many other files the Pack holds, which a rebuild would have had to
    /// carry forward.
    pub kept: usize,
    /// Why no rebuild happened: `key_lost` or `unverified`.
    pub reason: &'static str,
}

impl RefusedPack {
    /// The refusal as the browser is told it.
    pub fn of(refused: &coffret_device::RefusedPack) -> Self {
        Self {
            spared: refused.spared.clone(),
            kept: refused.kept.len(),
            reason: match refused.reason {
                PackRefusal::KeyLost => "key_lost",
                PackRefusal::Unverified(_) => "unverified",
            },
        }
    }

    /// Every refusal a preview counts.
    pub fn all_of(preview: &DeletePreview) -> Vec<Self> {
        preview.refused.iter().map(Self::of).collect()
    }

    /// The sentence a person reads beside the files that stayed.
    ///
    /// Written here rather than by the browser, as every refusal's sentence is:
    /// what a screen shows verbatim is the server's.
    pub fn message(&self) -> &'static str {
        match self.reason {
            "key_lost" => {
                "the Library has no key for the Pack holding them, so the files packed beside \
                 them cannot be carried into a rebuilt one — the Pack is left as it is"
            }
            _ => {
                "the Pack holding them did not verify when it was read back, so no rebuilt one \
                 was written — the Pack is left as it is"
            }
        }
    }
}
