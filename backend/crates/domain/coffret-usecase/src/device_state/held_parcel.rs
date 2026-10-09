use std::ops::Range;
use std::path::PathBuf;

use coffret_model::ContainerId;

/// One parcel of one Container whose ciphertext this device keeps on its own
/// disk (spec: PK-21).
///
/// Device state, never uploaded and never part of an Index Snapshot: what a
/// device holds is ciphertext it read, and losing every row of it costs a
/// re-read the provider can observe and nothing else. The row is what lets
/// that ciphertext be found again and, once it has served its purpose, let go
/// of — nothing lists the directory the files are in.
///
/// Which Entries the parcel covers is recorded as the stretch of the
/// Container's plaintext stream it opens into, rather than as the Entries
/// themselves: the catalog already says where each current Entry lies in its
/// Container (spec: CP-11), so the two together answer which Entries a parcel
/// has bytes of without the object's front being read again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldParcel {
    /// The Container the parcel is a part of.
    pub container_id: ContainerId,
    /// Which parcel it is, counted from 0 at the Container's first chunk
    /// (spec: PK-19).
    pub index: u64,
    /// The stretch of the Container's plaintext stream the parcel opens into.
    pub plaintext: Range<u64>,
    /// Where on this device the parcel's ciphertext is kept.
    ///
    /// Device state like a spool path: it never travels into a Container, a
    /// Journal record, or a diagnostic event.
    pub path: PathBuf,
}

impl HeldParcel {
    /// Whether an Entry standing at `extent` in the same Container has bytes in
    /// this parcel.
    ///
    /// An Entry of no bytes has none anywhere, and is taken to be covered by
    /// the parcel its position stands in or at the end of: the reader that
    /// places it reads that parcel (spec: PK-16), and keeping the parcel a
    /// moment longer for it costs nothing a later run does not let go of.
    pub fn covers(&self, offset: u64, size: u64) -> bool {
        if size == 0 {
            return self.plaintext.start <= offset && offset <= self.plaintext.end;
        }
        offset < self.plaintext.end && offset + size > self.plaintext.start
    }
}
