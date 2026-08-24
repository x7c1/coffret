//! The orders the arrays in a control-object payload are written in.
//!
//! Every array FM-15, FM-16, and FM-17 define is in a stated order — Container
//! IDs by their sixteen bytes, Entries by the canonical UTF-8 bytes of their
//! Entry Path (EP-3) — so that one Library state has exactly one encoding. Two
//! devices committing the same batch produce the same map, and a record does
//! not change its bytes because a writer happened to hold its additions in a
//! different order.
//!
//! Putting an array in that order is the encoder's job, and checking it is the
//! decoder's. A decoder that sorted a payload into shape instead would accept
//! two encodings of one state and hide the writer that produced the second.

use std::cmp::Ordering;

use crate::error::{Error, Result};

/// Rejects an array that is not strictly increasing under `compare`.
///
/// Strictly, not merely non-decreasing: the keys these arrays are ordered by
/// identify their elements — one Container ID names one Container, and one
/// Entry Path holds at most one current Entry at a committed state (EP-5) — so
/// a repeat is a payload naming something twice, which the same check catches.
pub(super) fn require_strictly_increasing<T>(
    array: &'static str,
    items: &[T],
    compare: impl Fn(&T, &T) -> Ordering,
) -> Result<()> {
    for index in 1..items.len() {
        if compare(&items[index - 1], &items[index]) != Ordering::Less {
            return Err(Error::ControlPayloadOutOfOrder { array, index });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_increasing_array_is_accepted() {
        require_strictly_increasing("containers", &[1, 2, 3], Ord::cmp)
            .expect("an increasing array is in order");
    }

    #[test]
    fn an_empty_or_single_array_is_in_order() {
        require_strictly_increasing("containers", &[0u8; 0], Ord::cmp).expect("empty is order");
        require_strictly_increasing("containers", &[7], Ord::cmp).expect("one is order");
    }

    #[test]
    fn the_first_element_out_of_order_is_named() {
        let result = require_strictly_increasing("entries", &[1, 5, 3, 9], Ord::cmp);
        assert!(
            matches!(
                result,
                Err(Error::ControlPayloadOutOfOrder {
                    array: "entries",
                    index: 2
                })
            ),
            "expected element 2 to be reported, got {result:?}"
        );
    }

    // A repeat is not "still sorted": it names one Container, or one Entry
    // Path, twice.
    #[test]
    fn a_repeated_key_is_rejected() {
        let result = require_strictly_increasing("containers", &[1, 2, 2], Ord::cmp);
        assert!(
            matches!(
                result,
                Err(Error::ControlPayloadOutOfOrder {
                    array: "containers",
                    index: 2
                })
            ),
            "expected a repeat to be rejected, got {result:?}"
        );
    }
}
