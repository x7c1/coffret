/// Whether the spool file a pending row names is a whole Container yet.
///
/// The row is written before the file it names exists, so that no ciphertext
/// this device produces is ever unaccounted for (spec: OC-2). That ordering is
/// what makes the distinction necessary: between the row and the flip that
/// follows the flush there is a file on disk no row calls a Container, and the
/// row has to be able to say so.
///
/// Only a row that is [`Writing`](Self::Writing) can become
/// [`Written`](Self::Written), and only by the spool step that finished the file
/// it names — nothing else moves a row between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PendingSpoolState {
    /// This device announced a spool file and may or may not have finished
    /// writing it.
    ///
    /// What is on disk may be nothing at all, part of a Container, or a whole
    /// one the run never got to record as whole — the row does not distinguish
    /// them because nothing needs to. Its content is worth nothing to anybody
    /// either way: no key for it was ever committed and nothing will ever open
    /// it. Only its disposal matters, which is what the row is for.
    Writing,
    /// The spool file holds a complete Container.
    ///
    /// The only kind that is ever uploaded or committed: a run puts a Container
    /// on Storage and names it in a batch only after the file it reads those
    /// bytes from is whole.
    Written,
}
