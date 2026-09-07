use std::ffi::OsString;

use crate::folder_entry_kind::FolderEntryKind;

/// One name a mapped folder holds, and what it stands for.
///
/// The name is the operating system's own bytes and not text, deliberately.
/// Turning a filename into something the Library can hold is a decision — EP-1's
/// normalization, EP-2's shape, and the refusal of a name that spells no Entry
/// Path at all — and it belongs above this capability, in the walk that knows
/// what to do with a name it cannot read. A gateway that lossily converted here
/// would be making that decision on the walk's behalf, and silently.
///
/// The name alone and not the path: what folder it was found in is what the
/// caller asked about, and joining is the caller's.
#[derive(Debug, Clone)]
pub struct FolderEntry {
    /// The name as the operating system spells it.
    pub name: OsString,
    /// What stands at that name, with links unfollowed (spec: EP-8).
    pub kind: FolderEntryKind,
}
