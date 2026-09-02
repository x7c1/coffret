//! How long a control object of each kind may be.
//!
//! A control object is read whole — it is one AEAD message, so there is no
//! opening part of one (spec: FM-11) — and how many bytes that costs is decided
//! by a number nothing has authenticated: the length Storage reports for the
//! object, or the length of whatever actually arrives. A reader that believed
//! either would let a provider, or anyone who wrote at the object's name, spend
//! a device's memory before the tag it would have failed was ever checked.
//!
//! So each kind carries a ceiling, derived from what that kind's schema can
//! actually produce for a Library far larger than any this format has met, with
//! room for the schema to grow. They are format decisions and live beside the
//! payload schemas they bound: what a Keyring costs per Container and what a
//! Snapshot costs per Entry are FM-17's and FM-16's answers, not a transport's.
//!
//! What they are not is a promise that an object of that size is workable. They
//! are the point past which a length is not a Library at all, and is refused
//! before anything is spent on it.

use coffret_model::{ControlObjectKind, ControlObjectName};

use crate::error::{Error, Result};

/// The longest Journal record this build reads or writes (spec: FM-15).
///
/// A record carries one commit's additions, and an addition carries the whole
/// entry table of the Container it adds — which is what lets a device replay a
/// record without opening a Container (spec: CP-11). So a record is sized by the
/// batch and not by the Library, and the largest batch is an initial import: one
/// `freeze` invocation over a whole folder tree.
///
/// At the ~120 bytes an Entry costs in a catalog payload (the design budget
/// FM-16's schema is measured against, and FM-15 spells the same entry map),
/// 256 MiB is a single commit of roughly two million Entries. A batch that size
/// is already a run of many hours; one past it is not a batch this format was
/// shaped for.
pub(super) const MAX_JOURNAL_RECORD_LEN: u64 = 256 * 1024 * 1024;

/// The longest Index Snapshot this build reads or writes, ordinary or
/// activation (spec: FM-16).
///
/// The Snapshot is the one payload that grows with the whole Library rather than
/// with a batch, and a device whose Index is older than the newest checkpoint
/// fetches one entire (spec: CK-9). At the schema's 120-byte design budget per
/// Entry, 512 MiB is a Library of some four million Entries — for a photo and
/// book collection, a decade of it several times over.
///
/// The ceiling is where the format's own shape gives out rather than where a
/// number looked round: a Library past it needs a checkpoint that can be read in
/// pieces, which is a change to FM-16 and not a larger constant. Raising this
/// one without that change would only move where the same memory is spent.
pub(super) const MAX_INDEX_SNAPSHOT_LEN: u64 = 512 * 1024 * 1024;

/// The longest Keyring replica this build reads or writes (spec: FM-17).
///
/// A Keyring maps every current Container to an envelope or a key-lost marker,
/// so it grows with the Container count — Containers, not Entries, which is why
/// its ceiling is the lowest of the three. At the ~110 bytes per Container the
/// schema is measured against, 64 MiB maps some six hundred thousand
/// Containers; at the gigabyte-scale Pack the size target aims for (spec: PK-5),
/// that is a Library measured in hundreds of terabytes.
///
/// Every generation is stored R times over and rewritten whole at each rotation
/// (spec: KL-8, MR-1), so this is also the one ceiling that bounds what a
/// rotation reads and writes repeatedly.
pub(super) const MAX_KEYRING_LEN: u64 = 64 * 1024 * 1024;

/// The longest object of one kind, header and tag included.
pub const fn max_control_object_len(kind: ControlObjectKind) -> u64 {
    match kind {
        ControlObjectKind::Journal => MAX_JOURNAL_RECORD_LEN,
        ControlObjectKind::Keyring => MAX_KEYRING_LEN,
        // An activation Snapshot is a Snapshot with two fields more (spec:
        // FM-16), so one envelope covers both kinds.
        ControlObjectKind::IndexSnapshot | ControlObjectKind::ActivationSnapshot => {
            MAX_INDEX_SNAPSHOT_LEN
        }
    }
}

/// The longest object a name may lead to, before its kind is known.
///
/// A reader asks this of the *name*, because that is all it has when it decides
/// how many bytes it is willing to take in: the kind rides in the header, and
/// the header is inside the answer. A name admits one kind or two (spec: FM-12),
/// and the answer is the larger of what it admits — refusing on the name alone
/// would refuse a legitimate object of the other kind.
pub fn max_control_object_len_at(name: &ControlObjectName) -> u64 {
    ControlObjectKind::ALL
        .iter()
        .filter(|kind| name.admits(**kind))
        .map(|kind| max_control_object_len(*kind))
        .max()
        // Every name in FM-12's table admits at least one kind. A name that
        // admitted none could lead to no object this build would open, so
        // nothing is worth reading for it.
        .unwrap_or(0)
}

/// Refuses a control-object length past its kind's ceiling.
///
/// Called with a length that has been *declared* — by Storage, or by the bytes
/// in hand — and never with one that has been authenticated, which is the whole
/// point: it is what a reader consults before spending memory on the claim, and
/// what a writer consults before laying out an object no reader would take.
pub(super) fn check_control_object_len(kind: ControlObjectKind, len: u64) -> Result<()> {
    let limit = max_control_object_len(kind);
    if len > limit {
        return Err(Error::ControlObjectTooLong { kind, len, limit });
    }
    Ok(())
}
