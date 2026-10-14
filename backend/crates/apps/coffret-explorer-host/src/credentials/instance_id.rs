use std::fmt;

/// The public name of one generation of the host: one run of it and of the
/// server behind it, serving one Library (spec: LA-18).
///
/// A value drawn as the host starts, with the generation counted onto it, so
/// that it differs across hosts and across the generations of one. It admits
/// nothing, and is shown to the page so that the page can tell one
/// generation's answers from the next one's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceId {
    pub(super) host: String,
    pub(super) generation: u64,
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.host, self.generation)
    }
}
