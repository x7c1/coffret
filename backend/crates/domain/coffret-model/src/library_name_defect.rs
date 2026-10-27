use std::fmt;

/// What is wrong with a piece of text as a Library's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryNameDefect {
    /// Nothing was given.
    Empty,
    /// It holds a control character, which no name should carry.
    Control,
}

impl fmt::Display for LibraryNameDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("it is empty"),
            Self::Control => f.write_str("it holds a control character"),
        }
    }
}
