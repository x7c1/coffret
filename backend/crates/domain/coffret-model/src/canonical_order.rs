//! The order every collection a control aggregate carries is held in.
//!
//! Container IDs by their sixteen bytes, Entries by the canonical UTF-8 bytes of
//! their Entry Path (spec: EP-3): the orders FM-15, FM-16, and FM-17 give the
//! arrays they define, stated here once so that the aggregates holding those
//! collections, the encoders writing them out, and the decoders reading them
//! back cannot disagree about what the order is.
//!
//! Every one of them is strict. The keys identify their elements — one
//! Container ID names one Container, one Entry Path holds at most one current
//! Entry at a committed state (spec: EP-5) — so a repeat is a collection naming
//! something twice, and the same walk catches it.

use std::cmp::Ordering;

use crate::error::{Error, Result};

/// The `additions` of a Journal record, ordered by Container ID (spec: FM-15).
pub(crate) const ADDITIONS: &str = "additions";
/// The `removals` of a Journal record, ordered by Container ID (spec: FM-15).
pub(crate) const REMOVALS: &str = "removals";
/// An Index Snapshot's `containers`, ordered by Container ID (spec: FM-16).
pub(crate) const CONTAINERS: &str = "containers";
/// An Index Snapshot's `entries`, ordered by Entry Path (spec: FM-16, EP-3).
pub(crate) const ENTRIES: &str = "entries";
/// A Keyring's `mapping`, ordered by Container ID (spec: FM-17).
pub(crate) const MAPPING: &str = "mapping";

/// Refuses a collection that is not strictly increasing under `compare`.
///
/// # Errors
///
/// [`Error::CollectionOutOfCanonicalOrder`], naming the first element that does
/// not follow its predecessor.
pub(crate) fn require_strictly_increasing<T>(
    collection: &'static str,
    items: &[T],
    compare: impl Fn(&T, &T) -> Ordering,
) -> Result<()> {
    for index in 1..items.len() {
        if compare(&items[index - 1], &items[index]) != Ordering::Less {
            return Err(Error::CollectionOutOfCanonicalOrder { collection, index });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The three shapes the walk has to tell apart: an order it accepts, the
    // first element out of it, and a repeat — which is not "still sorted" but a
    // collection naming one thing twice.
    #[test]
    fn a_collection_is_in_order_when_every_element_follows_its_predecessor() {
        require_strictly_increasing(CONTAINERS, &[1, 2, 3], Ord::cmp)
            .expect("an increasing collection is in order");
        require_strictly_increasing(CONTAINERS, &[0u8; 0], Ord::cmp).expect("empty is order");
        require_strictly_increasing(CONTAINERS, &[7], Ord::cmp).expect("one is order");

        assert!(
            matches!(
                require_strictly_increasing(ENTRIES, &[1, 5, 3, 9], Ord::cmp),
                Err(Error::CollectionOutOfCanonicalOrder {
                    collection: "entries",
                    index: 2,
                })
            ),
            "the first element out of order is the one named",
        );
        assert!(
            matches!(
                require_strictly_increasing(CONTAINERS, &[1, 2, 2], Ord::cmp),
                Err(Error::CollectionOutOfCanonicalOrder {
                    collection: "containers",
                    index: 2,
                })
            ),
            "a repeat is refused by the same walk",
        );
    }
}
