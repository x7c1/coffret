/// Which kind of user-data Container this is.
///
/// The kind is recorded explicitly rather than inferred from the entry count: a
/// Pack left holding a single Entry is still a Pack, and a replacement for a
/// one-file Container is still one-file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerKind {
    /// Created by uploading a single file on its own.
    OneFile,
    /// Created by the pack policy — freeze, repack, or compaction.
    Pack,
}
