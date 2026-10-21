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
    /// Its name is not text, and so spells no Entry Path (spec: EP-2).
    NotText,
    /// The operating system would not say what it is, or list it.
    Unreadable,
}
