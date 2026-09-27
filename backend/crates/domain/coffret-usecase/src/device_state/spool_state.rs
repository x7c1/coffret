use coffret_model::ObjectRef;

/// Whether the spool file a pending row names is a whole Container yet.
///
/// The row is written before the file it names exists, so that no ciphertext
/// this device produces is ever unaccounted for (spec: OC-2). That ordering is
/// what makes the distinction necessary: between the row and the flip that
/// follows the flush there is a file on disk no row calls a Container, and the
/// row has to be able to say so.
///
/// Only a row that is [`Spooling`](Self::Spooling) can become
/// [`Spooled`](Self::Spooled), and only by the spool step that finished the file
/// it names — nothing else moves a row between the two.
///
/// The object handle lives inside [`Spooled`](Self::Spooled) because a
/// Container is uploaded only after its spool is complete: a `Spooling` row has
/// no object, and the type leaves no way to write one that does.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpoolState {
    /// This device announced a spool file and may or may not have finished
    /// writing it.
    ///
    /// What is on disk may be nothing at all, part of a Container, or a whole
    /// one the run never got to record as whole — the row does not distinguish
    /// them because nothing needs to. Its content is worth nothing to anybody
    /// either way: no key for it was ever committed and nothing will ever open
    /// it. Only its disposal matters, which is what the row is for.
    Spooling,
    /// The spool file holds a complete Container.
    ///
    /// The only kind that is ever uploaded or committed: a run puts a Container
    /// on Storage and names it in a batch only after the file it reads those
    /// bytes from is whole.
    ///
    /// It carries where the Container was uploaded to, once it has been.
    /// `None` means the ciphertext exists only in the spool, so abandoning the
    /// batch removes a local file and nothing on Storage.
    Spooled(Option<ObjectRef>),
}

impl SpoolState {
    /// Where the Container was uploaded to, if it has been.
    ///
    /// Always `None` for a [`Spooling`](Self::Spooling) row, which by
    /// construction has never been uploaded.
    #[must_use]
    pub const fn object_ref(&self) -> Option<&ObjectRef> {
        match self {
            Self::Spooling | Self::Spooled(None) => None,
            Self::Spooled(Some(object)) => Some(object),
        }
    }

    /// Whether the spool file holds a complete Container.
    #[must_use]
    pub const fn is_spooled(&self) -> bool {
        matches!(self, Self::Spooled(_))
    }
}
