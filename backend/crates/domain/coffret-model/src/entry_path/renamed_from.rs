use super::EntryPath;

impl EntryPath {
    /// Where this path stands once the subtree at `from` is renamed to `to`, or
    /// `None` where it is not in that subtree.
    ///
    /// `from` itself becomes `to`, and a path under `from/` keeps the rest of
    /// itself under `to/` — the same subtree [`is_under`](Self::is_under) says
    /// `from` covers, so `books` never reaches `books-annex/page-1.png`. Like
    /// [`below`](Self::below) it owes no check: the rest of a shaped path past a
    /// whole leading component is shaped, and joining two shaped paths with the
    /// one separator is too (spec: EP-2).
    pub fn renamed_from(&self, from: &Self, to: &Self) -> Option<Self> {
        if self == from {
            return Some(to.clone());
        }
        self.0
            .strip_prefix(&from.0)
            .and_then(|rest| rest.strip_prefix('/'))
            .map(|rest| Self(format!("{}/{rest}", to.0)))
    }
}
