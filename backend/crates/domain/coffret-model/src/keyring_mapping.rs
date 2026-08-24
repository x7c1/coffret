use crate::keyring_entry::KeyringEntry;

/// The complete mapping one Keyring generation carries (spec: KL-6, KL-7).
///
/// Every replica of a generation carries this same mapping, which is why
/// reading needs one valid replica and the replica count adds redundancy
/// rather than a quorum (spec: KL-6). At every commit and `prune` boundary the
/// committed mapping covers every current Container and no other; whether a
/// caller's mapping does is the caller's obligation (spec: KL-7), and holding
/// the entries is all this type does.
///
/// The order the entries are held in carries no meaning: the wire order is
/// Container ID order, and the encoder puts them in it (spec: FM-17). That is
/// what makes one mapping one byte string and therefore one `set_digest`,
/// whichever device wrote it (spec: KL-1, KL-14).
///
/// This is the mapping's content as a domain value. How it is encoded,
/// digested, encrypted under a purpose key, and framed as a control object is
/// the format layer's business (spec: FM-11, FM-17).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyringMapping {
    /// The Containers this generation maps, in no order the caller has to keep.
    pub entries: Vec<KeyringEntry>,
}

impl KeyringMapping {
    /// Takes the entries a generation maps, in whatever order they were held.
    pub const fn new(entries: Vec<KeyringEntry>) -> Self {
        Self { entries }
    }
}
