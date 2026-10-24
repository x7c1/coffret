use std::error;
use std::fmt;

/// What is wrong with a hex spelling a manifest states.
#[derive(Debug)]
pub enum HexDefect {
    /// An odd number of characters, which spells no whole byte string.
    OddLength {
        /// How many characters there are.
        len: usize,
    },
    /// A character that is not a lowercase hex digit.
    NotADigit {
        /// The character.
        found: char,
    },
    /// A byte string of another length than the value it spells takes.
    WrongLength {
        /// How many bytes the value takes.
        expected: usize,
        /// How many the spelling spells.
        found: usize,
    },
}

impl fmt::Display for HexDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OddLength { len } => {
                write!(f, "expected an even number of hex characters, found {len}")
            }
            Self::NotADigit { found } => {
                write!(f, "expected a lowercase hex character, found {found:?}")
            }
            Self::WrongLength { expected, found } => {
                write!(f, "expected {expected} bytes, found {found}")
            }
        }
    }
}

impl error::Error for HexDefect {}
