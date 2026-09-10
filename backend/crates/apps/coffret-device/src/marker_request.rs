/// Whether recording a mapping may leave the root's identity as it found it
/// (spec: EP-13).
///
/// Ordinary operation never issues a new identity: a root that already carries a
/// marker keeps it, so that several mappings — or several devices — sharing one
/// root share one identity and none of them destroys another's. Wanting a new
/// one is a real wish all the same, and the case it answers is two roots that
/// ended up with the same identifier because one was copied from the other. The
/// rule is that asking for it is explicit, which is what this type makes it: a
/// caller says which of the two it means, and neither is the silent default of
/// the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerRequest {
    /// Keep whatever identity the root already carries, and write one only
    /// where the root carries none.
    AdoptWhatIsThere,
    /// Replace the identity a valid marker carries with a freshly drawn one.
    ///
    /// It replaces an identity and does not repair a broken management area: a
    /// marker that is malformed, over the cap, or not a regular file is an error
    /// with this asked for exactly as it is without.
    IssueANewIdentity,
}
