use std::error;
use std::fmt;

/// What made a cached token file unreadable.
///
/// The two are one verdict to a caller — the cache is no good, so authorize
/// again — and are kept apart only so that whichever layer saw the failure has
/// its own answer travel whole, rather than a message composed here about work
/// this layer did not do.
#[derive(Debug)]
pub enum TokenCacheDefect {
    /// The sealed form could not be opened: another Master Key wrote it, its
    /// bytes have been edited, or it was never a sealed cache at all.
    ///
    /// Every one of those is a fact about the file, which is what makes this a
    /// verdict on the cache. The one thing the format layer refuses that is a
    /// fact about the *caller* — a key derived for another purpose — never
    /// reaches here: [`Error::WrongTokenCacheKey`] is raised before the file is
    /// so much as opened.
    ///
    /// [`Error::WrongTokenCacheKey`]: super::Error::WrongTokenCacheKey
    Sealed(coffret_format::Error),
    /// The sealed form opened, and what was inside is not the token document
    /// this build expects.
    Document(serde_json::Error),
}

impl fmt::Display for TokenCacheDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the two it was, and not what the format layer or the
            // reader answered: `source` hands that value on, and a caller
            // printing the chain reads it under this line.
            Self::Sealed(_) => f.write_str("the sealed form could not be opened"),
            Self::Document(_) => {
                f.write_str("what was inside is not the token document this build expects")
            }
        }
    }
}

impl error::Error for TokenCacheDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Sealed(cause) => Some(cause),
            Self::Document(cause) => Some(cause),
        }
    }
}
