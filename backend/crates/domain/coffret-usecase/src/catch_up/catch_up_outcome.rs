use coffret_model::Generation;

/// What one catalog catch-up moved.
///
/// Two readings of the same run, because they answer two different questions. The
/// generations say whether the Library moved at all — a head this device had not
/// seen is the whole of what a catch-up is looking for — and the Entry counts say
/// what that came to for somebody looking at a folder. A Library can commit a
/// record that adds nothing they would notice (a removal, a Container replaced by
/// an identical one), and a caller reading only the counts would call that "up to
/// date" when the catalog it is showing has in fact changed.
///
/// Nothing here is about this device's own files. A catch-up places nothing and
/// fetches nothing, so every row it added is `remote` until something asks for its
/// bytes (spec: EP-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatchUpOutcome {
    /// The head this device's catalog stood at before the run, and `None` on one
    /// that stood at no committed state — a device that has just joined
    /// (spec: CK-9, RV-1).
    pub from: Option<Generation>,
    /// The head it stands at now, and `None` in a Library that has committed
    /// nothing (spec: FM-13).
    pub to: Option<Generation>,
    /// How many current Entries the catalog held before the run.
    pub entries_before: usize,
    /// How many it holds now.
    pub entries_after: usize,
}

impl CatchUpOutcome {
    /// Whether the catalog moved to a head it had not seen.
    ///
    /// A catch-up never goes backwards — it starts from the newer of this Index
    /// and the newest checkpoint and replays forwards (spec: CK-9) — so this is a
    /// comparison rather than a difference, and a device that had seen nothing is
    /// below every head there is.
    pub fn advanced(&self) -> bool {
        self.to > self.from
    }

    /// How many current Entries the catalog gained, which is negative where
    /// another device's commit removed more than it added.
    ///
    /// A count and never a list: what changed is the listing's to say, folder by
    /// folder, and a run that replayed a thousand records would otherwise be
    /// asked to carry a thousand paths through a status line.
    pub fn gained(&self) -> i64 {
        self.entries_after as i64 - self.entries_before as i64
    }
}
