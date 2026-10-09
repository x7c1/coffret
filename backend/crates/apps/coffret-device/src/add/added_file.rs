use coffret_model::{EntryPath, Mtime};

/// One file standing in a mapped folder that the Library holds no Entry for.
///
/// A row about this device and not about the Library, which is what makes it
/// unlike every other row a listing carries: there is no Entry behind it, no
/// Container holding it, and no size or modification time the Library preserved
/// (spec: FM-9) — what is here is what the filesystem answered a moment ago.
///
/// Two things put a file in this state, and to this listing they are the same
/// state. One is a file just added to the folder, which the next sync will carry
/// in; the other is a file whose Entry left the Library while this device's copy
/// stayed on disk — departed, which the next sync moves to the trash or, where
/// the copy changed, keeps and reports rather than carrying back in
/// (spec: EP-15). Neither is in the Library now, and the answer for both is the
/// same: it is here, and the Library does not have it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedFile {
    /// Its last path component, which is what it is called.
    pub name: String,
    /// Where in the Library it would stand (spec: EP-1, EP-9).
    pub path: EntryPath,
    /// Its length on disk in bytes.
    pub size: u64,
    /// Its modification time on disk.
    pub mtime: Mtime,
}
