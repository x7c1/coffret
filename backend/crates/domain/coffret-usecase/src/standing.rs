use coffret_model::Mtime;

/// What stands at a local path now, read without following a symbolic link.
///
/// The three things a writer has to know before it may claim a path, and no
/// more: how long what is there is, when it was last changed, and whether it is
/// an ordinary file at all (spec: EP-10, EP-11). The last is not a detail — a
/// symbolic link is not the file it points at, which is the same reading the
/// scan makes of one (spec: EP-8) — so anything that is not a regular file is
/// still *something in the way*.
///
/// It is what [`Destinations::look_up`](crate::Destinations::look_up) answers
/// with, so it is public: a gateway outside this crate is what reads a
/// filesystem's own metadata, and this is the form the answer comes back in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// Its length in bytes.
    pub size: u64,
    /// Its modification time, in whole seconds (spec: FM-9).
    pub mtime: Mtime,
    /// Whether it is an ordinary file rather than a folder, a symbolic link, or
    /// anything else.
    pub is_file: bool,
}
