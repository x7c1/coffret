//! The one place the fixture literals become generations.
//!
//! Every number the format carries is bounded (spec: FM-19), so a generation is
//! read rather than taken on trust — and the generator's numbers are the
//! constants in the source above, not anything a run was handed. So a refusal
//! here is a typo in the fixture set rather than a state any exchange could
//! reach, and it is said as one, the way `entry_paths` says it for a path.

use coffret_model::Generation;

/// The generation a fixture literal names, or a panic naming the literal that
/// names none.
pub(super) fn generation(number: u64) -> Generation {
    Generation::new(number)
        .unwrap_or_else(|error| panic!("a fixture holds a literal this crate wrote: {error}"))
}
