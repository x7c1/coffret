use coffret_model::Generation;
use tracing::warn;

/// A committed Keyring set a read had to step over positions of to reach a
/// valid replica: the set is short, to that read, at every position it found
/// no valid replica (spec: KL-5, KL-15).
///
/// Short rather than degraded, because a run knows the set only through what it
/// read. Degraded is the set's own state, and a finding says it only where the
/// read established a loss (see [`established`](Self::established)).
///
/// Carried out of the read rather than logged inside it, because a run that
/// goes on to write examines the same generation in its commit, and that
/// examination says what it found and what it put back (spec: KL-11, KL-15) —
/// two lines about one set, from one run, where the read had already written
/// its own. So the finding waits, and only a run that reached no examination
/// reports it. A run that did not commit also hands it to its caller on its
/// outcome, because the person who ran a flow that only reads is the one KL-15
/// is about.
///
/// Two counts rather than one, because the positions a walk steps over are of
/// two kinds and only one of them is a loss. A replica that is absent, does not
/// open, is not a Keyring, or carries another key table than its name promises
/// is one the set has lost: that is the degraded state KL-5 names. A replica
/// Storage would not hand over is one nothing is known about — the next run
/// may read it without trouble — so it is not known to be lost, and a finding
/// made of those alone says that whether the set is degraded is not established
/// rather than overstating it (spec: KL-15).
///
/// The counts are floors rather than tallies: the replicas above the one that
/// answered are never fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortKeyring {
    generation: Generation,
    replicas: u16,
    lost: u16,
    unfetched: u16,
}

impl ShortKeyring {
    /// A finding about `generation`'s set of `replicas`, where the walk found
    /// `lost` positions lost and `unfetched` more Storage did not hand over.
    ///
    /// Public so that a caller's own cases can put together the finding an
    /// outcome carries; a read is the only thing in this crate that makes one.
    pub fn new(generation: Generation, replicas: u16, lost: u16, unfetched: u16) -> Self {
        Self {
            generation,
            replicas,
            lost,
            unfetched,
        }
    }

    /// The committed generation whose set the read walked.
    pub fn generation(&self) -> Generation {
        self.generation
    }

    /// How many replicas the commitment declares (spec: KL-2).
    pub fn replicas(&self) -> u16 {
        self.replicas
    }

    /// How many positions the read found lost: absent, unreadable, not a
    /// Keyring, or carrying another key table (spec: KL-1, KL-5).
    pub fn lost(&self) -> u16 {
        self.lost
    }

    /// How many positions Storage did not hand over, so that nothing about
    /// them is known.
    pub fn unfetched(&self) -> u16 {
        self.unfetched
    }

    /// Whether the read established that the set is degraded — that at least
    /// one position is lost rather than merely not handed over (spec: KL-5).
    pub fn established(&self) -> bool {
        self.lost > 0
    }

    /// Says what the read found: the set is degraded and awaits a writer's
    /// repair where a loss is established, and otherwise only that Storage
    /// did not hand some replicas over.
    ///
    /// Counts, positions and a generation, and nothing else: what a person
    /// asked for reaches them on their terminal, and a diagnostic event carries
    /// none of it (spec: EL-1, EL-3).
    pub(crate) fn report(&self) {
        if self.established() {
            warn!(
                generation = self.generation.get(),
                replicas = self.replicas,
                lost = self.lost,
                unfetched = self.unfetched,
                "the committed Keyring is degraded and awaits repair by a writer",
            );
        } else {
            warn!(
                generation = self.generation.get(),
                replicas = self.replicas,
                unfetched = self.unfetched,
                "Storage did not hand over some replicas of the committed Keyring, so whether \
                 any of them is lost is not established",
            );
        }
    }
}
