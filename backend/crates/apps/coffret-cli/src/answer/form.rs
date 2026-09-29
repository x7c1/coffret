//! Which form a run answers in.

/// Which form a run answers in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Sentences for a person, as a run has always printed them.
    Text,
    /// One JSON object for a script.
    Json,
}

impl Form {
    /// Whether what a person reads on standard output is to be printed.
    pub fn is_text(self) -> bool {
        self == Self::Text
    }
}
