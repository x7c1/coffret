use std::path::PathBuf;

/// Where the explorer's page and its assets are served from.
#[derive(Debug, Clone)]
pub enum PageSource {
    /// What was compiled into the binary under the `embed-web` feature, and
    /// without it a short page saying the explorer is not in this binary. What
    /// every shipped binary serves.
    Embedded,
    /// The explorer's build output in a directory on disk, read as each request
    /// asks for it, so that a build that rewrote it is served on the next
    /// reload. For a development host in front of a checkout.
    ///
    /// Only what is inside the directory is served: a path naming a dotfile or
    /// stepping outside it is refused before the disk is asked, and so is a
    /// path that reaches a symbolic link on the way down, which would
    /// otherwise serve whatever it points at.
    Directory(PathBuf),
}
