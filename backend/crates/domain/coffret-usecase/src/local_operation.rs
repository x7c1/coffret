use std::fmt;

/// What a local file or folder was being asked for when the operating system
/// refused.
///
/// A value rather than the word that goes in the message: a caller telling
/// somebody what to go and fix has a different sentence for a spool it could
/// not write than for a source file it could not read, and finding out which
/// happened by matching on prose is not something a public error type may ask
/// of it. The word itself is this type's [`Display`](fmt::Display), which is
/// where a rendering belongs.
///
/// It is shared by the two flows that touch this device's own disks — the sync
/// that reads a folder into the Library and the fetch that writes one back out
/// — because what the operating system refused is one vocabulary whichever
/// direction the bytes were going.
///
/// There is deliberately no `PartialEq`, for the reason the error types
/// carrying it have none.
#[derive(Debug, Clone, Copy)]
pub enum LocalOperation {
    /// A directory's entries were being read.
    Listing,
    /// A directory entry's own metadata was being read, links unfollowed
    /// (spec: EP-8).
    Stating,
    /// A source file's plaintext was being read.
    Reading,
    /// A spool file, a fetch's temporary file, or a directory above one was
    /// being made.
    Creating,
    /// Ciphertext or fetched plaintext was going into a file.
    Writing,
    /// A file was being flushed to the device, so that it outlasts the run that
    /// wrote it (spec: OC-2).
    Flushing,
    /// A fetched file's modification time was being set to its Entry's
    /// (spec: FM-9, EP-11).
    Stamping,
    /// A fully verified fetch was being moved onto its final local path
    /// (spec: EP-11).
    Renaming,
    /// A spool file whose Container was committed or abandoned, or a temporary
    /// file a failed fetch left, was being deleted (spec: OC-6, EP-11).
    Removing,
}

impl fmt::Display for LocalOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Listing => "listed",
            Self::Stating => "stated",
            Self::Reading => "read",
            Self::Creating => "created",
            Self::Writing => "written",
            Self::Flushing => "flushed",
            Self::Stamping => "stamped",
            Self::Renaming => "renamed",
            Self::Removing => "removed",
        })
    }
}
