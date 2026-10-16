use std::sync::Arc;

use coffret_model::{KeyTable, KeyringCommitment};
use tracing::debug;

use super::{FetchMemory, Flight, Turn};
use crate::commit::{CommitError, ShortKeyring};

/// What one read of the committed key table came to, and what it found of the
/// set.
pub(crate) type KeyTableRead = Result<(Arc<KeyTable>, Option<ShortKeyring>), CommitError>;

/// What a reader of the committed key table is to do.
pub(crate) type KeyTableTurn<'m> = Turn<
    'm,
    (Arc<KeyTable>, Option<ShortKeyring>),
    (KeyringCommitment, KeyTableGeneration),
    KeyTableRead,
>;

/// Which forgetting of the key table a read of the set began after.
///
/// A read takes it before it reads the replicas and hands it back with the
/// table it read, and a table read under any but the latest is not kept: a
/// forget landed while the read was under way, so what it found of the set may
/// be what a repair has since made untrue (spec: KL-13, KL-15).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyTableGeneration(u64);

/// One committed key table, and the commitment it was read under.
#[derive(Debug)]
pub(super) struct Opened {
    commitment: KeyringCommitment,
    key_table: Arc<KeyTable>,
    short: Option<ShortKeyring>,
}

impl FetchMemory {
    /// The key table read under `commitment`, and what that read found of the
    /// set, where it was read; a wait on the read of it under way, where one
    /// began after the last forget; and the read to make, where neither is.
    ///
    /// A read that began before the last forget is not joined: what it finds
    /// of the set may be what a repair has since made untrue, which is why it
    /// will not be kept either (spec: KL-15). The read to make comes with the
    /// generation it begins under, for
    /// [`remember_key_table`](Self::remember_key_table).
    pub(crate) fn key_table_for(
        &self,
        commitment: &KeyringCommitment,
    ) -> (KeyTableTurn<'_>, KeyTableGeneration) {
        let mut kept = self.locked();
        let read_under = kept.forgotten;
        if let Some(opened) = kept
            .keyring
            .as_ref()
            .filter(|opened| &opened.commitment == commitment)
        {
            let found = (Arc::clone(&opened.key_table), opened.short);
            return (Turn::Kept(found), read_under);
        }
        let key = (commitment.clone(), read_under);
        if let Some(waiter) = kept.key_table_reads.join(&key) {
            return (Turn::Join(waiter), read_under);
        }
        let flight = Flight::start(self, &mut kept, |kept| &mut kept.key_table_reads, key);
        (Turn::Read(flight), read_under)
    }

    /// Forgets the key table where the read that produced it found the set
    /// short, and keeps it where it found the set whole.
    ///
    /// What a catch-up does: it is where this process asks Storage what the
    /// Library is now, and so where a repair another device made since is
    /// learned of (spec: KL-13). The commitment a repair leaves is the one it
    /// found, so the finding would otherwise outlive the condition it reports;
    /// the next fetch reads the set again instead, and says it is short only if
    /// it still is (spec: KL-15).
    ///
    /// Advances the generation whether or not it forgets anything: a read
    /// under way across the catch-up may be about to keep a finding of a short
    /// set that a repair the catch-up learned of has made untrue.
    pub(crate) fn forget_short_key_table(&self) {
        let mut kept = self.locked();
        kept.forgotten.0 += 1;
        if kept
            .keyring
            .as_ref()
            .is_some_and(|opened| opened.short.is_some())
        {
            kept.keyring = None;
        }
    }

    /// Forgets the key table, whatever its read found of the set.
    ///
    /// What a write run of this process does once it is over: it examines the
    /// committed set and repairs it before it commits (spec: KL-11, KL-13), and
    /// a run that repaired and then failed leaves the commitment where it was
    /// with the set whole again. The next fetch reads the set rather than
    /// reporting a finding the run has made untrue (spec: KL-15).
    ///
    /// And a fetch that read the set before the repair and is still on its
    /// way does not put that finding back: the generation this advances is
    /// past the one it read under, so what it hands back to be kept is not.
    pub fn forget_key_table(&self) {
        let mut kept = self.locked();
        kept.forgotten.0 += 1;
        kept.keyring = None;
    }

    /// The generation a read of the set begins under, taken before it reads
    /// any replica and handed back to
    /// [`remember_key_table`](Self::remember_key_table) with what it read.
    #[cfg(test)]
    fn key_table_generation(&self) -> KeyTableGeneration {
        self.locked().forgotten
    }

    /// Keeps the key table read under `commitment`, in place of any other —
    /// unless a forget landed since the read began at `read_under`.
    ///
    /// A table read before a forget is what the set was before whatever the
    /// forget answers for, and a write run of this process forgets once it has
    /// repaired the set (spec: KL-13). Kept, it would have the next fetch report
    /// the set short after the repair made it whole (spec: KL-15); left, the
    /// next fetch reads the set again. The table still serves the fetch that
    /// read it, which is the caller's own business.
    pub(crate) fn remember_key_table(
        &self,
        read_under: KeyTableGeneration,
        commitment: KeyringCommitment,
        key_table: Arc<KeyTable>,
        short: Option<ShortKeyring>,
    ) {
        let mut kept = self.locked();
        if kept.forgotten != read_under {
            debug!("the key table was read before it was last forgotten, so it is not kept");
            return;
        }
        kept.keyring = Some(Opened {
            commitment,
            key_table,
            short,
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use coffret_model::{Generation, KeyringCommitment};

    use super::{FetchMemory, Turn};

    /// Whether the memory keeps a key table for the case's commitment.
    fn kept_table(memory: &FetchMemory) -> bool {
        matches!(memory.key_table_for(&commitment()).0, Turn::Kept(_))
    }

    fn commitment() -> KeyringCommitment {
        KeyringCommitment::new(Generation::FIRST, 1, "beef").expect("a valid commitment")
    }

    // A write run forgets the key table once it has repaired the set
    // (spec: KL-13), and a fetch that read the set before that and keeps what
    // it read after would put the finding back (spec: KL-15). The table that
    // read hands back is ignored; the next fetch reads the set again.
    #[test]
    fn a_table_read_before_the_last_forget_is_not_kept() {
        let memory = FetchMemory::default();
        let read_under = memory.key_table_generation();

        memory.forget_key_table();
        memory.remember_key_table(read_under, commitment(), Arc::default(), None);

        assert!(!kept_table(&memory));

        // And a read that began after it is kept as before.
        let read_under = memory.key_table_generation();
        memory.remember_key_table(read_under, commitment(), Arc::default(), None);
        assert!(kept_table(&memory));
    }

    // The catch-up's forget is one too, whether or not the table it finds is
    // one it drops: a read under way across it may be about to keep a finding
    // the catch-up has learned is no longer true.
    #[test]
    fn a_catch_up_advances_the_generation_too() {
        let memory = FetchMemory::default();
        let read_under = memory.key_table_generation();

        memory.forget_short_key_table();
        memory.remember_key_table(read_under, commitment(), Arc::default(), None);

        assert!(!kept_table(&memory));
    }
}
