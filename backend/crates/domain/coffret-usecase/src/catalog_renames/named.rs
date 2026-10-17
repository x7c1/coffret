use coffret_model::EntryLocation;

/// What a catalog holds under one `(Container ID, entry number)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Named {
    /// The Container is not in the current set.
    NoContainer,
    /// The Container is current and its entry table holds no Entry at that
    /// number (spec: FM-9).
    NoEntry,
    /// The Entry, where it stands now.
    Entry(Box<EntryLocation>),
}
