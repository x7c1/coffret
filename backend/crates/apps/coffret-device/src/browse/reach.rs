use std::collections::BTreeSet;

use coffret_model::EntryPath;
use coffret_usecase::device_state::Mapping;

/// Which parts of the Library this device has a folder for (spec: EP-9).
///
/// The mappings partition the Library's namespace between them: a top-level
/// mapping represents its own subtree, and a Library-root mapping represents
/// everything the top-level ones do not. So whether one folder is reachable is
/// decided by its top-level component alone, and this is the two facts about
/// the mappings that decide it.
pub(super) struct Reach {
    /// Whether a mapping stands at the Library root. With one present nothing
    /// is out of reach, since it represents whatever no other mapping does.
    root: bool,
    /// The top-level components a mapping of their own represents.
    represented: BTreeSet<String>,
}

impl Reach {
    /// What this device's mappings come to, as the question a listing asks.
    ///
    /// Owned rather than borrowed from the mappings, because the listing holds
    /// the answer past the call that read them.
    pub(super) fn of(mappings: Vec<Mapping>) -> Self {
        Self {
            root: mappings.iter().any(|mapping| mapping.prefix.is_none()),
            represented: Mapping::represented_prefixes(&mappings)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    }

    /// Whether a mapping reaches one folder, `None` being the Library root.
    ///
    /// The root is reached by the root mapping alone: a top-level mapping
    /// stands for its own subtree and not for what sits beside it, so a device
    /// that maps only `albums` has nowhere to put a file that lives at the top
    /// of the Library. Every other folder is reached by whichever mapping
    /// represents its top-level component, or by the root mapping where none
    /// does.
    pub(super) fn reaches(&self, folder: Option<&EntryPath>) -> bool {
        match folder {
            None => self.root,
            Some(path) => self.root || self.represented.contains(path.top_level()),
        }
    }
}
