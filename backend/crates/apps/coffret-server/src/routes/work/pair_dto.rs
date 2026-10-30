use coffret_device::RenamePair;
use serde::Serialize;

/// One `(from, to)` of a move, as the wire spells it.
#[derive(Debug, Serialize)]
pub(crate) struct PairDto {
    /// The Entry Path moved: a file, or a folder with everything under it.
    from: String,
    /// The Entry Path it is moved to.
    to: String,
}

impl PairDto {
    pub(crate) fn of(pair: &RenamePair) -> Self {
        Self {
            from: pair.from.as_str().to_owned(),
            to: pair.to.as_str().to_owned(),
        }
    }
}
