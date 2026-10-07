use crate::canonical_order::{require_strictly_increasing, MAPPING};
use crate::error::Result;
use crate::keyring_element::KeyringElement;

/// The complete key table one Keyring generation carries (spec: KL-6, KL-7).
///
/// Every replica of a generation carries this same key table, which is why
/// reading needs one valid replica and the replica count adds redundancy
/// rather than a quorum (spec: KL-6). At every commit and `prune` boundary the
/// committed key table covers every current Container and no other; whether a
/// caller's key table does is the caller's obligation (spec: KL-7), and holding
/// the elements is all this type does.
///
/// What it does hold to is that the elements are in Container ID order and name
/// each Container once (spec: FM-17). That is one rule with two faces: the
/// order is what makes one key table one byte string and therefore one
/// `set_digest`, whichever device wrote it (spec: KL-1, KL-14), and strictness
/// is what keeps a key table from carrying two answers for one Container. A
/// caller holding elements in the order it happened to gather them sorts through
/// [`canonical`](Self::canonical) rather than handing them over unsorted.
///
/// This is the key table's content as a domain value. How it is encoded — as
/// the payload field `mapping` — digested, encrypted under a purpose key, and
/// framed as a control object is the format layer's business (spec: FM-11,
/// FM-17).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyTable {
    elements: Vec<KeyringElement>,
}

impl KeyTable {
    /// The key table `elements` spell, or a refusal where they are not in the
    /// order FM-17 writes them in.
    ///
    /// # Errors
    ///
    /// [`Error::CollectionOutOfCanonicalOrder`](crate::Error::CollectionOutOfCanonicalOrder)
    /// where `elements` is not strictly increasing by Container ID — an order
    /// the encoding does not admit, or one Container mapped twice.
    pub fn new(elements: Vec<KeyringElement>) -> Result<Self> {
        require_strictly_increasing(MAPPING, &elements, |left, right| {
            left.container_id.cmp(&right.container_id)
        })?;
        Ok(Self { elements })
    }

    /// The same key table from elements in whatever order a writer gathered them:
    /// sorted by Container ID, then held to [`new`](Self::new)'s rule.
    ///
    /// Sorting cannot make a Container mapped twice disappear, so what this
    /// refuses is exactly what `new` refuses once the order is no longer in
    /// question (spec: FM-17, KL-7).
    ///
    /// # Errors
    ///
    /// [`Error::CollectionOutOfCanonicalOrder`](crate::Error::CollectionOutOfCanonicalOrder)
    /// where two elements name one Container.
    pub fn canonical(mut elements: Vec<KeyringElement>) -> Result<Self> {
        elements.sort_by_key(|element| element.container_id);
        Self::new(elements)
    }

    /// The Containers this generation maps, in the Container ID order FM-17
    /// fixes.
    pub fn elements(&self) -> &[KeyringElement] {
        &self.elements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use crate::testing::keyring_element;

    // FM-17: the key table is ordered by Container ID and strictly so, because a
    // generation that mapped one Container twice would hold two answers for it
    // — which KL-7's "exactly one" rules out.
    #[test]
    fn a_key_table_naming_a_container_twice_cannot_exist() {
        let result = KeyTable::new(vec![
            keyring_element(1),
            keyring_element(2),
            keyring_element(2),
        ]);

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
                KeyTable::new(vec![keyring_element(2), keyring_element(1)]),
                Err(Error::CollectionOutOfCanonicalOrder {
                    collection: "mapping",
                    index: 1,
                })
            ),
            "and a key table out of Container ID order with it",
        );
    }

    // An empty key table is a key table: a Library that has committed no
    // Container maps none, which is what `Default` stands for.
    #[test]
    fn a_key_table_of_no_containers_is_a_key_table() {
        assert!(KeyTable::default().elements().is_empty());
        KeyTable::new(Vec::new()).expect("an empty key table is in order");
    }
}
