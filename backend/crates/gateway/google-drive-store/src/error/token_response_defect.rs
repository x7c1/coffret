use std::error;
use std::fmt;

/// What kept the token endpoint's answer from being read.
///
/// As with [`TokenCacheDefect`], the two are one verdict to a caller — nothing
/// usable came back from the endpoint — and are kept apart so that whichever
/// layer saw the failure has its own answer travel whole.
///
/// [`TokenCacheDefect`]: super::TokenCacheDefect
#[derive(Debug)]
pub enum TokenResponseDefect {
    /// The body never arrived whole: the transfer broke, or it was not as
    /// long as the answer declared.
    Body(coffret_usecase::Error),
    /// The body arrived, and what it holds is not the token document this
    /// build expects.
    Document(serde_json::Error),
}

impl fmt::Display for TokenResponseDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the two it was, for the reason [`TokenCacheDefect`]'s
            // two say only which of theirs it was.
            Self::Body(_) => f.write_str("the body never arrived whole"),
            Self::Document(_) => {
                f.write_str("what the body holds is not the token document this build expects")
            }
        }
    }
}

impl error::Error for TokenResponseDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Body(cause) => Some(cause),
            Self::Document(cause) => Some(cause),
        }
    }
}
