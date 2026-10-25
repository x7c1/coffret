use std::fmt;

use coffret_model::Redacted;

/// Why a handle took nothing.
#[derive(Debug, Clone, Copy)]
pub enum DropRefusal {
    /// No drop is held under it: it was never drawn, it was already used, or
    /// a later drop ended it.
    Unknown,
    /// The drop it named was held longer than
    /// [`DROP_LIFETIME`](super::DROP_LIFETIME).
    Expired,
}

impl fmt::Display for DropRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unknown => "no drop is held under that handle",
            Self::Expired => "the drop that handle named is no longer held",
        })
    }
}

impl std::error::Error for DropRefusal {}

impl Redacted for DropRefusal {
    /// Which refusal it is, and nothing else.
    ///
    /// No variant carries a name, so its identity is the whole of what a
    /// diagnostic event is told.
    fn redacted(&self) -> String {
        match self {
            Self::Unknown => "DropRefusal::Unknown",
            Self::Expired => "DropRefusal::Expired",
        }
        .to_owned()
    }
}
