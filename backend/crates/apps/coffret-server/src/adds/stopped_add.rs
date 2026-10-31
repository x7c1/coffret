use crate::folder::Folder;

/// What an add a person stopped came to, as the log keeps it (spec: LA-36).
///
/// Counts and the folder it was made onto. A stop is answered with what is
/// so on this device afterwards: how many of the add's files were taken away,
/// how many a run had already committed — a stop that met its run committing
/// is answered by that run instead, so these are the files a run under way
/// beside it had carried in — and how many are still in the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoppedAdd {
    /// The folder the add was made onto.
    pub folder: Folder,
    /// How many files it had written.
    pub files: usize,
    /// How many of them were taken away from the folder: removed, or, where
    /// one had been written over a file of the person's, replaced by that
    /// file again.
    pub removed: usize,
    /// How many of them a run had committed before the stop was answered;
    /// they are the Library's, and stay.
    pub committed: usize,
    /// How many are still in the folder for any other reason: one another
    /// file was put in the place of since it landed, or one this device would
    /// not remove or put the person's file back over.
    pub left: usize,
}
