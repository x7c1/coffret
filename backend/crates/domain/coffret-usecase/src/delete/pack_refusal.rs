use crate::read_modify_replace::Unverified;

/// Why read-modify-replace could not replace a Container (spec: PK-10).
#[derive(Debug)]
pub enum PackRefusal {
    /// The committed Keyring maps the Container to a key-lost marker, so its
    /// Entries cannot be read back and none can be carried forward
    /// (spec: KL-7, KL-17).
    ///
    /// Not a fault to retry: deleting every Entry of the Container would remove
    /// it, and `update` from a surviving local copy, or a recovered envelope,
    /// is what makes its Entries readable again (spec: PK-11, RV-8).
    KeyLost,
    /// The Container was read and did not verify (spec: PK-10).
    ///
    /// A replacement is never written from bytes that did not verify, and
    /// never written missing a kept Entry.
    Unverified(Unverified),
}
