use std::fmt;

/// Which part of the shape an Entry Path is held to a piece of text fails
/// (spec: EP-2).
///
/// A refusal that said only "that is not an Entry Path" would leave whoever
/// typed it to guess which of the variants below they broke, so the one that
/// failed travels with the refusal and reaches a person in words they can act
/// on.
///
/// Naming the defect and not the text: a rendering that has to keep Library
/// content out of it can still say this much, which is what the
/// [`Redacted`](crate::Redacted) rendering of
/// [`Error::MalformedEntryPath`](crate::Error::MalformedEntryPath) does with
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathDefect {
    /// Nothing was given, and an Entry Path names a position in a Library.
    Empty,
    /// It holds a NUL, which no name a filesystem can spell carries.
    Nul,
    /// It begins with `/`, which would make it absolute rather than a position
    /// in a Library.
    LeadingSeparator,
    /// It ends with `/`, which leaves the last component nameless.
    TrailingSeparator,
    /// Two separators run together somewhere inside it, so one component is
    /// empty.
    EmptyComponent,
    /// One component is `.` or `..`, which names a directory relative to
    /// another rather than a position of its own.
    RelativeComponent,
}

impl fmt::Display for PathDefect {
    /// What a person reading a refusal is told, as the second half of a
    /// sentence whose first half named the path.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("it is empty"),
            Self::Nul => f.write_str("it holds a NUL"),
            Self::LeadingSeparator => f.write_str(
                "it begins with a separator, and an Entry Path is relative to the Library root",
            ),
            Self::TrailingSeparator => f.write_str("it ends with a separator"),
            Self::EmptyComponent => f.write_str("it holds an empty component"),
            Self::RelativeComponent => f.write_str("it holds a `.` or `..` component"),
        }
    }
}
