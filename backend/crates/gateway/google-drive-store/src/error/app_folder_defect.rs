use std::error;
use std::fmt;

/// What kept an app folder from being created, read, or looked into.
///
/// As with [`TokenCacheDefect`], every one of them is one verdict to a caller —
/// there is no folder to work in — and they are kept apart so that whichever
/// layer saw the failure has its own answer travel whole. None of that depends
/// on how many there are, and saying the number here would be one more thing
/// for whoever adds the next variant to keep true.
///
/// [`TokenCacheDefect`]: super::TokenCacheDefect
#[derive(Debug)]
pub enum AppFolderDefect {
    /// The call never succeeded: Drive refused it, or its answer never arrived
    /// whole. It comes classified — whether trying again could help is carried
    /// in it.
    Call(coffret_usecase::Error),
    /// Drive answered, and the answer is not the file resource this build
    /// expects, so nothing in it names a folder.
    Answer(serde_json::Error),
    /// Drive answered with a file resource carrying no name, though the name is
    /// the one field the call asked for.
    Nameless,
    /// Drive kept handing back somewhere to carry on and never said the listing
    /// was over, so the walk stopped instead of following it forever.
    ///
    /// Apart from [`Call`](Self::Call) and [`Answer`](Self::Answer) by what was
    /// observed: every call succeeded and every answer read as a listing, and
    /// what is wrong is the sequence of them. Apart from
    /// [`Nameless`](Self::Nameless), which this layer also puts together
    /// itself, by what it is about — a field Drive left out of one answer,
    /// against a walk over many that never reached an end. A provider that
    /// answers an empty page and another continuation without end would
    /// otherwise leave the call spinning, which a person sees as a command that
    /// never returns.
    UnendingListing {
        /// How many pages were read before the walk gave up.
        pages: usize,
    },
}

impl fmt::Display for AppFolderDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the three it was, and not what the call or the reader
            // answered: `source` hands that value on, and a caller printing
            // the chain reads it under this line rather than twice over.
            //
            // This one is about the call and the two below are about an
            // answer, which is the whole of what tells them apart: Drive
            // refusing outright and Drive sending back something this build
            // cannot read are different things to be told. A line saying the
            // call was not answered with a folder would be true of all three
            // and would leave a person guessing which had happened.
            Self::Call(_) => f.write_str("the call to Drive did not succeed"),
            Self::Answer(_) => {
                f.write_str("the answer is not the file resource this build expects")
            }
            Self::Nameless => {
                f.write_str("the answer carries no name, which is what was asked for")
            }
            // The one arm whose line is about the listing rather than about a
            // call or an answer, so it says how far the walk got: the number
            // is the whole of what a reader can check the verdict against.
            Self::UnendingListing { pages } => {
                write!(f, "the listing did not end within {pages} pages")
            }
        }
    }
}

impl error::Error for AppFolderDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Call(cause) => Some(cause),
            Self::Answer(cause) => Some(cause),
            // Nothing a Rust error reported: a name Drive left out and a
            // listing that would not end are both facts this layer put
            // together itself.
            Self::Nameless | Self::UnendingListing { .. } => None,
        }
    }
}
