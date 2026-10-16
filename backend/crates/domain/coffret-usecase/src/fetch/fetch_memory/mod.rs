use std::sync::{Mutex, MutexGuard, PoisonError};

use coffret_model::{ContainerId, Generation, KeyringCommitment};

mod flights;
pub(crate) use flights::Turn;
use flights::{Flight, Flights};

mod front;
use front::FrontRead;

mod fronts;
use fronts::Fronts;

mod key_table;
use key_table::{KeyTableGeneration, KeyTableRead, Opened};

mod listing;
use listing::{Listed, ListingRead};

/// What a process that fetches many Entries keeps between them, so that the
/// second one does not ask Storage again for what the first already learned.
///
/// Every Entry fetch needs three things before it reads a parcel: the control
/// objects the catalog's head was read from (the handles a Keyring replica and a
/// Container this device never uploaded are reached by, spec: FM-3, FM-12), the
/// committed key table (spec: KL-1, KL-6), and the front of the Container it
/// reads (spec: FM-2, PK-16). None of the three changes between two fetches that
/// stand on one catalog, and a process serving a reader asks for many Entries
/// over one catalog. So it keeps them, each for exactly as long as it stays
/// true:
///
/// - **The listing** is kept with the head the catalog stood at when it was
///   taken. A catalog standing anywhere else — a catch-up advanced it, or a
///   commit of this device's own did — is one the listing may not cover, and
///   the next fetch lists again.
/// - **The key table** is kept with the commitment it was read under, and only
///   handed back for that commitment (spec: KL-1). A catalog whose checkpoint
///   names another Keyring has its fetch read that one. Beside it is what the
///   read found of the set. A repair puts replicas back under the same
///   commitment (spec: KL-13, KL-14), so a table whose read found the set
///   short is forgotten at the next catch-up, and any table once a write run
///   of this process — where this device repairs — is over; the next fetch
///   reads the set again and reports it short only if that read finds it so
///   (spec: KL-15). Every forget advances a generation a read of the set takes
///   before it starts, and a table read under an older one is not kept, so a
///   fetch that read the set before a repair cannot put the finding back after
///   it. A table read from a whole set is kept across a catch-up that leaves
///   the commitment where it was.
/// - **A Container's front** is kept by Container ID, within a bound on how many
///   and how many bytes, and forgotten once the Container leaves the current
///   set. A Container object never changes once committed (spec: FM-2, CP-11),
///   so the bytes are the object's own for as long as it is current. What is
///   kept is the ciphertext as it arrived; it is opened under the Container's
///   key each time it is used, exactly as a front read from Storage is, and a
///   front served from here shows the provider less than a read would, never
///   more (spec: PK-20).
///
/// The key table holds envelopes wrapped under the Library's wrap key, and the
/// fronts hold sealed meta sections: neither opens anything without the keys.
/// It is kept beside the keys all the same, and goes when they do — a device
/// that has locked holds nothing a reader could use (spec: DK-2, DK-4). That is
/// why it is a value the open Library carries rather than one the process
/// does: the lock drops it with the keys, and the unlock starts from nothing.
///
/// It keeps, beside what has been read, what is being read: a fetch that wants
/// the listing, the key table or a front another is reading already — a page
/// opened while its folder's fronts are being read ahead, say — waits for that
/// read rather than making it a second time. The first reader reads and lands
/// what it came to, failure included, for every reader waiting on it; a reader
/// dropped before it lands anything takes its read off the ones under way, and
/// whoever was waiting on it reads for itself.
///
/// Nothing here is a reason to read anything. A fetch handed this answers from
/// the catalog as it stands and catches nothing up by itself; the catalog moves
/// when its owner catches it up, and records what it read here on the way
/// (see [`CatchUpRequest::remembering`](crate::catch_up::CatchUpRequest::remembering)).
#[derive(Debug, Default)]
pub struct FetchMemory {
    kept: Mutex<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    listing: Option<Listed>,
    keyring: Option<Opened>,
    /// Advanced by every forget of the key table, whether or not one was kept.
    forgotten: KeyTableGeneration,
    fronts: Fronts,
    /// The listings being read, by the head they are read at.
    listing_reads: Flights<Option<Generation>, ListingRead>,
    /// The key tables being read, by the commitment they are read under and
    /// the forgetting the read began after.
    key_table_reads: Flights<(KeyringCommitment, KeyTableGeneration), KeyTableRead>,
    /// The fronts being read, by Container.
    front_reads: Flights<ContainerId, FrontRead>,
}

impl FetchMemory {
    /// How many readers wait on another's read, across every kind.
    #[cfg(all(test, feature = "conformance"))]
    pub(crate) fn waiting(&self) -> usize {
        let kept = self.locked();
        kept.listing_reads.waiting() + kept.key_table_reads.waiting() + kept.front_reads.waiting()
    }

    fn locked(&self) -> MutexGuard<'_, Kept> {
        // Every write under this lock replaces one field or edits one
        // collection (the fronts' queue, a kind's reads under way) in one
        // step, so what a panic left behind is still a memory — at worst one
        // that forgot something and reads it again.
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
