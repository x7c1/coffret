/// What recording a mapping did to the marker standing in its root
/// (spec: EP-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerRecord {
    /// The root carried no management area, so one was made and a freshly drawn
    /// identity written into it.
    Written,
    /// The root already carried a valid marker, and its identity was adopted
    /// without the file being rewritten.
    Adopted,
    /// A valid marker was replaced with a freshly drawn identity, because that
    /// was asked for outright ([`MarkerRequest::IssueANewIdentity`]).
    ///
    /// [`MarkerRequest::IssueANewIdentity`]: crate::MarkerRequest::IssueANewIdentity
    Reset,
}
