use crate::reserved_names::ReservedName;

/// Why the walk did not take something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotTakenReason {
    /// Nothing was dropped there: an empty path, or one naming no item in a
    /// folder.
    Empty,
    /// It was not there by the time the walk looked.
    Missing,
    /// It is neither a regular file nor a folder — a symbolic link most of all,
    /// which is never followed (spec: EP-8).
    NotAFileOrFolder,
    /// Its name is one coffret keeps for itself inside a mapped folder
    /// (spec: EP-11, EP-14).
    Reserved(ReservedName),
    /// Its name is one an operating system keeps beside the person's own —
    /// `.DS_Store`, `Thumbs.db`, a `._` file, a `__MACOSX` folder — and nothing
    /// under a folder of one was walked (spec: EP-17).
    ///
    /// Not a refusal like the others: such a name is in almost every drop, so
    /// the page says it apart and quietly rather than as something gone wrong.
    System {
        /// Whether a folder stood at the name.
        folder: bool,
    },
    /// Its name is not text, and so spells no Entry Path (spec: EP-2).
    NotText,
    /// The operating system would not say what it is, or list it.
    Unreadable,
}
