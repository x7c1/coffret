use std::path::PathBuf;

/// What one look at the server and the launch file came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    /// Nothing that a person is to be told.
    Quiet,
    /// The server was seen for the first time, and the launch file written:
    /// the path to open.
    Opened(PathBuf),
    /// The server's key changed, so a server of another run is behind the
    /// port: the generation was retired, every session with it, and a fresh
    /// launch file written at the path to open again (spec: LA-4, LA-18).
    ServerChanged(PathBuf),
    /// The file's ticket was spent, or old enough that it could expire while
    /// it is being opened, and a fresh one stands in its place.
    Rearmed,
    /// The file was removed and none stands in its place, because the server
    /// is not answering. The next one is written once it is.
    Withdrawn,
}
