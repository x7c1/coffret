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
/// It is shared by the three flows that touch this device's own disks — the
/// sync and the freeze that read a folder into the Library and the fetch that
/// writes one back out — because what the operating system refused is one
/// vocabulary whichever direction the bytes were going. Every call the flows
/// make into that disk is a gateway's, behind one of the three capabilities
/// over it, and answers in this vocabulary through
/// [`LocalIoError`](crate::LocalIoError) — as does a caller outside them that
/// keeps files of its own on the same disk, since one refusal about a local
/// file has one shape whoever asked for it.
///
/// There is deliberately no `PartialEq`, for the reason the error types
/// carrying it have none.
#[derive(Debug, Clone, Copy)]
pub enum LocalOperation {
    /// A directory's entries were being read.
    ///
    /// The directory's own listing and nothing below it: a listing states every
    /// child as it reads the name, and a child whose own record is what refused
    /// is [`Stating`](Self::Stating) on that child's path. So a caller reporting
    /// a refused listing never puts one file's path next to a sentence about
    /// the folder, and never sends a person to look at the wrong thing.
    Listing,
    /// A file's own metadata was being read: a directory entry's with links
    /// unfollowed (spec: EP-8), and a mapped root's following them, the way
    /// [`Listing`](Self::Listing) would resolve it anyway (spec: EP-12) — and a
    /// placement's own open of one before that root vouches for itself, stated
    /// rather than created because a placement never makes the root
    /// (spec: EP-13).
    Stating,
    /// A source file's plaintext was being read, or a finished spool was being
    /// opened to be sent to Storage (spec: OC-2) — and a mapped root's marker
    /// file was being opened and read, which is what says which folder the root
    /// is (spec: EP-13).
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
    /// A lock was being taken on a file a device keeps for itself, so that one
    /// process at a time holds what that file stands for.
    ///
    /// The lock already being held is not this. That is an answer rather than a
    /// failure, and whoever asked for the lock says in its own words what it
    /// means; this is the operating system declining to arbitrate at all.
    Locking,
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
            Self::Locking => "locked",
        })
    }
}
