use crate::below_root_error::BelowRootError;

/// What [`Destination::keep_aside`](crate::Destination::keep_aside) found at
/// the file's own name, and what it did about it.
///
/// A writer adding a file to a mapped folder replaces whatever file stands at
/// its name: that is what the rename that publishes it does (spec: EP-11). An
/// add can be stopped before its run commits, and stopping it undoes it on this
/// device (spec: LA-36), which for a file it wrote over means putting back the
/// file that stood there. So before the rename, the file is kept aside — by a
/// second link to it, under a scratch name in the same folder — and this is
/// what came of that.
pub enum Aside {
    /// Nothing a publish would replace stands at the name: no file at all, or
    /// a folder, which the rename refuses to replace.
    Empty,
    /// A file stood there, and it is now under a scratch name of its own too.
    Kept(Box<dyn KeptFile>),
    /// A file stood there and could not be kept aside: the filesystem makes no
    /// hard link, or will not make one more to this file. Stopping the add that
    /// writes over it could not put it back, which is that add's to say.
    Unkept(BelowRootError),
}

impl std::fmt::Debug for Aside {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("Empty"),
            Self::Kept(_) => f.write_str("Kept"),
            Self::Unkept(refused) => f.debug_tuple("Unkept").field(refused).finish(),
        }
    }
}

/// A file kept aside under a scratch name in the folder it stood in, by a
/// second link to it (spec: LA-36, OC-8).
///
/// It holds the folder it is in open, as the [`Destination`](crate::Destination)
/// that made it does, so both calls below are made against that folder by name
/// and neither resolves a path again (spec: EP-4, EP-11).
///
/// Neither call is made by the value on its own: what it is kept for is the
/// caller's account to keep, and that caller says when the file goes back and
/// when it is let go.
pub trait KeptFile: Send + Sync {
    /// Puts the kept file back at the file's own name, replacing what stands
    /// there now, and leaves nothing under the scratch name.
    ///
    /// A rename within one folder, so what a scan or a reader sees at that name
    /// is the file it replaced or the one put back, never neither
    /// (spec: EP-11). Whether what stands there now is still the file that took
    /// its place is the caller's to have asked first.
    ///
    /// # Errors
    ///
    /// [`BelowRootError::Io`] carrying
    /// [`Renaming`](crate::LocalOperation::Renaming) where the rename was
    /// refused; the kept file is still under its scratch name then.
    fn put_back(&self) -> Result<(), BelowRootError>;

    /// Removes the scratch name, and with it the second link: what is left is
    /// whatever stands at the file's own name.
    ///
    /// A name that is already gone is the outcome this wanted (spec: OC-8).
    /// Synchronous, for the reason
    /// [`Destination::remove`](crate::Destination::remove) is: it is called as
    /// the value that holds it is dropped.
    ///
    /// # Errors
    ///
    /// [`BelowRootError::Io`] carrying
    /// [`Removing`](crate::LocalOperation::Removing).
    fn remove(&self) -> Result<(), BelowRootError>;
}
