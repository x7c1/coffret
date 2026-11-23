use coffret_model::Mtime;

/// What the whole Library comes to, as the Index holds it: the cached
/// Catalog's Entries and Containers, and this device's present records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibrarySummary {
    /// How many current Entries the Library holds, at every depth.
    pub files_total: u64,
    /// How many of those this device has (spec: EP-10).
    pub files_present: u64,
    /// The plaintext bytes of every current Entry together.
    pub size: u64,
    /// The plaintext bytes of the `files_present` Entries together.
    pub size_present: u64,
    /// The latest modification time among every current Entry, `None` for a
    /// Library holding none.
    pub mtime: Option<Mtime>,
    /// How many current Containers the catalog holds.
    pub containers: u64,
}
