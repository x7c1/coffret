use crate::canonical_order::{require_strictly_increasing, MAPPING};
use crate::error::Result;
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
/// What it does hold to is that the entries are in Container ID order and name
/// each Container once (spec: FM-17). That is one rule with two faces: the
/// order is what makes one mapping one byte string and therefore one
/// `set_digest`, whichever device wrote it (spec: KL-1, KL-14), and strictness
/// is what keeps a mapping from carrying two answers for one Container. A
/// caller holding entries in the order it happened to gather them sorts through
/// [`canonical`](Self::canonical) rather than handing them over unsorted.
///
/// This is the mapping's content as a domain value. How it is encoded,
/// digested, encrypted under a purpose key, and framed as a control object is
/// the format layer's business (spec: FM-11, FM-17).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyringMapping {
    entries: Vec<KeyringEntry>,
}

impl KeyringMapping {
    /// The mapping `entries` spell, or a refusal where they are not in the
    /// order FM-17 writes them in.
    ///
    /// # Errors
    ///
    /// [`Error::CollectionOutOfCanonicalOrder`](crate::Error::CollectionOutOfCanonicalOrder)
    /// where `entries` is not strictly increasing by Container ID — an order
    /// the encoding does not admit, or one Container mapped twice.
    pub fn new(entries: Vec<KeyringEntry>) -> Result<Self> {
        require_strictly_increasing(MAPPING, &entries, |left, right| {
            left.container_id.cmp(&right.container_id)
        })?;
        Ok(Self { entries })
    }

    /// The same mapping from entries in whatever order a writer gathered them:
    /// sorted by Container ID, then held to [`new`](Self::new)'s rule.
    ///
    /// Sorting cannot make a Container mapped twice disappear, so what this
    /// refuses is exactly what `new` refuses once the order is no longer in
    /// question (spec: FM-17, KL-7).
    ///
    /// # Errors
    ///
    /// [`Error::CollectionOutOfCanonicalOrder`](crate::Error::CollectionOutOfCanonicalOrder)
    /// where two entries name one Container.
    pub fn canonical(mut entries: Vec<KeyringEntry>) -> Result<Self> {
        entries.sort_by_key(|entry| entry.container_id);
        Self::new(entries)
    }

    /// The Containers this generation maps, in the Container ID order FM-17
    /// fixes.
    pub fn entries(&self) -> &[KeyringEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::testing::keyring_entry;

    // FM-17: the mapping is ordered by Container ID and strictly so, because a
    // generation that mapped one Container twice would hold two answers for it
    // — which KL-7's "exactly one" rules out.
    #[test]
    fn a_keyring_mapping_naming_a_container_twice_cannot_exist() {
        let result =
            KeyringMapping::new(vec![keyring_entry(1), keyring_entry(2), keyring_entry(2)]);

        assert!(
            matches!(
                result,
                Err(Error::CollectionOutOfCanonicalOrder {
                    collection: "mapping",
                    index: 2,
                })
            ),
            "expected the repeat to be refused, got {result:?}",
        );
        assert!(
            matches!(
                KeyringMapping::new(vec![keyring_entry(2), keyring_entry(1)]),
                Err(Error::CollectionOutOfCanonicalOrder {
                    collection: "mapping",
                    index: 1,
                })
            ),
            "and a mapping out of Container ID order with it",
        );
    }

    // An empty mapping is a mapping: a Library that has committed no Container
    // maps none, which is what `Default` stands for.
    #[test]
    fn a_mapping_of_no_containers_is_a_mapping() {
        assert!(KeyringMapping::default().entries().is_empty());
        KeyringMapping::new(Vec::new()).expect("an empty mapping is in order");
    }
}
