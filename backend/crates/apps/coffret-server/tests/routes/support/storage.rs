//! Storage and the catalog, as a case can take them away, give them back and
//! count what reached them.

use coffret_model::{ControlObjectName, ReplicaPosition};
use coffret_usecase::{ByteStream, Index};

use super::Served;

impl Served {
    /// How many reads asked for a range of an object, since the fixture was built.
    pub fn ranged_reads(&self) -> usize {
        self.reads.ranged_reads()
    }

    /// Makes the served device's catalog refuse to say what it maps, from now
    /// on.
    ///
    /// The question every door onto a file on this device asks first, so this
    /// is a catalog that could not be used, met wherever a route asks it.
    pub fn refuse_the_catalog(&self) {
        self.catalog.refuse();
    }

    /// Takes Storage away, as an unreachable bucket or a grant that ran out.
    pub fn halt_storage(&self) {
        self.storage.halt();
    }

    /// Gives it back.
    pub fn resume_storage(&self) {
        self.storage.resume();
    }

    /// How many reads Storage refused while it was away.
    pub fn refused_reads(&self) -> usize {
        self.storage.refused()
    }

    /// Takes every read and answers none of it, until it is let go.
    ///
    /// What a case uses this for is a request it knows is inside the server: it
    /// waits for [`held_reads`](Self::held_reads) to move, does whatever it is
    /// about to the server, and then lets the read go and reads the answer.
    pub fn hold_storage(&self) {
        self.storage.hold();
    }

    /// Lets the held read go.
    pub fn release_storage(&self) {
        self.storage.release();
    }

    /// How many reads are being, or have been, held.
    pub fn held_reads(&self) -> usize {
        self.storage.held_reads()
    }

    /// Leaves Storage reachable and mute, as a filtered network does.
    ///
    /// The other half of [`halt_storage`](Self::halt_storage): nothing is
    /// refused, and nothing is answered either, so whatever asked waits until it
    /// decides not to. [`resume_storage`](Self::resume_storage) is how it stops.
    pub fn stall_storage(&self) {
        self.storage.stall();
    }

    /// How many reads Storage was asked for and never answered.
    pub fn stalled_reads(&self) -> usize {
        self.storage.stalled_reads()
    }

    /// Mangles the first replica of the Library's committed Keyring set, so
    /// every read of the set from now on steps over a position it has
    /// established as lost and reads the next (spec: KL-5, KL-6).
    ///
    /// Written straight into the store, behind the served device's switch and
    /// counter: a Keyring replica that went bad is a fact about Storage, and
    /// no device did it. The files still open (spec: RV-2), which is what lets
    /// a case over this ask what a reader is told rather than whether it can
    /// read.
    pub async fn degrade_the_keyring(&self) {
        let checkpoint = self
            .filled
            .checkpoint()
            .await
            .expect("the filling device's catalog answers")
            .expect("the filling device committed");
        let committed = checkpoint.keyring();
        assert!(
            committed.replica_count() >= 2,
            "a set to degrade has a second position to fall back onto (spec: KL-8)",
        );
        let first = ReplicaPosition::new(0, committed.replica_count())
            .expect("the first position of a committed set is a position");
        let name = ControlObjectName::keyring_replica(
            committed.generation(),
            committed.set_digest(),
            first,
        )
        .expect("a committed digest is a valid one")
        .to_string();
        self.store
            .put(
                &name,
                ByteStream::from(b"not a Keyring replica at all".to_vec()),
            )
            .await
            .expect("the replica is overwritten");
    }
}
