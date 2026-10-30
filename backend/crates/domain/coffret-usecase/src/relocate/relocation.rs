use coffret_model::EntryPath;

use crate::relocate::relocation_refused::RelocationRefused;

/// What became of one file this device placed whose Entry a rename has given
/// another current path (spec: EP-10).
///
/// A run that relocates reports every waiting file it acted on or could not:
/// one relocated, discarded, or one that left this device is work done, one
/// refused is a file still standing at a name the Library no longer gives it,
/// which a person may have to act on.
///
/// The Entry Paths travel in the value because the caller is what decides what
/// to do about them. They never travel into a diagnostic event.
///
/// There is deliberately no `PartialEq`, for the reason
/// [`RelocationRefused`] has none.
#[derive(Debug, Clone)]
pub enum Relocation {
    /// The file was relocated from where it stood to its Entry's current path with
    /// one rename, and its record followed it.
    Relocated {
        /// Where the file stood.
        from: EntryPath,
        /// Where it stands now, its Entry's current path.
        to: EntryPath,
    },
    /// The Entry's current path already held this device's own copy of it —
    /// a fetch placed it there while this file waited — so this one, still as
    /// this device placed it, was discarded into the desktop's trash and its
    /// record forgotten (spec: EP-15).
    Discarded {
        /// Where the discarded file stood.
        from: EntryPath,
        /// Where the copy that stands is.
        to: EntryPath,
    },
    /// The file could not follow its Entry on this device — no mapped folder
    /// of this device reaches the Entry's current path, or the folder that does
    /// stands on another filesystem than the file — so this one, still as this
    /// device placed it, was discarded into the desktop's trash and its record
    /// forgotten: the Entry is in the Library and not on this device at its
    /// current path, and opening it there fetches it again (spec: EP-10,
    /// EP-15).
    Left {
        /// Where the discarded file stood.
        from: EntryPath,
        /// Where its Entry stands now.
        to: EntryPath,
    },
    /// The file was left where it stands, its record still waiting, for the
    /// reason given; the next run that relocates tries again.
    Refused {
        /// Where the file stands.
        from: EntryPath,
        /// Where its Entry stands now.
        to: EntryPath,
        /// Why it was not relocated.
        reason: RelocationRefused,
    },
}

impl Relocation {
    /// Where the file stood when the run came to it.
    pub fn from(&self) -> &EntryPath {
        match self {
            Self::Relocated { from, .. }
            | Self::Discarded { from, .. }
            | Self::Left { from, .. }
            | Self::Refused { from, .. } => from,
        }
    }

    /// Where its Entry stands now.
    pub fn to(&self) -> &EntryPath {
        match self {
            Self::Relocated { to, .. }
            | Self::Discarded { to, .. }
            | Self::Left { to, .. }
            | Self::Refused { to, .. } => to,
        }
    }
}
