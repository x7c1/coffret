//! The one place this crate's tests and conformance suites turn a literal pair
//! of numbers into an [`EntryExtent`].
//!
//! Every extent is built by asking whether the pair places an Entry inside the
//! plaintext stream's address space (spec: FM-9), and a fixture is no
//! exception: what a suite writes down is a pair of numbers like any other, and
//! the type has no constructor that takes an arbitrary one on trust. The unwrap
//! lives here rather than at each of the fixtures so that a pair somebody
//! mistypes is reported once, as the mistake in the fixture that it is.

use coffret_model::EntryExtent;

/// The extent `offset` and `size` place an Entry at, or a panic naming the pair
/// that places none.
pub(crate) fn entry_extent(offset: u64, size: u64) -> EntryExtent {
    EntryExtent::new(offset, size)
        .unwrap_or_else(|error| panic!("a fixture holds a literal extent: {error}"))
}
