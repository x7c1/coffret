//! The one place this crate's tests and conformance suites turn a literal
//! number into a [`Generation`].
//!
//! A generation is a number the format bounds (spec: FM-19), so the type has no
//! constructor that takes one on trust; a fixture is no exception. The unwrap
//! lives here rather than at each of the fixtures so that a number somebody
//! mistypes is reported once, as the mistake in the fixture that it is.

use coffret_model::Generation;

/// The generation `number` names, or a panic naming the literal that names
/// none.
pub(crate) fn generation(number: u64) -> Generation {
    Generation::new(number)
        .unwrap_or_else(|error| panic!("a fixture holds a literal generation: {error}"))
}
