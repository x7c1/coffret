use coffret_model::EntryPath;

/// Which way from a bound a lookup among one folder's children looks, in EP-3
/// order (spec: EP-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The nearest child before the bound, or the last one where there is no
    /// bound.
    Before,
    /// The nearest child after the bound, or the first one where there is no
    /// bound.
    After,
}

impl Side {
    /// The nearest of `paths` on this side of `bound`, or the first or last of
    /// them where there is no bound; `paths` are siblings, in EP-3 order.
    ///
    /// Siblings share every component but their last, so their EP-3 order is
    /// the byte order of their whole paths, which is what `bound` is compared
    /// by. The bound itself is on neither side.
    pub(crate) fn nearest(
        self,
        paths: impl IntoIterator<Item = EntryPath>,
        bound: Option<&EntryPath>,
    ) -> Option<EntryPath> {
        let beyond = |path: &EntryPath| match bound {
            None => true,
            Some(bound) => match self {
                Self::Before => path.as_str() < bound.as_str(),
                Self::After => path.as_str() > bound.as_str(),
            },
        };
        let mut paths = paths.into_iter().filter(|path| beyond(path));
        match self {
            Self::Before => paths.last(),
            Self::After => paths.next(),
        }
    }
}
