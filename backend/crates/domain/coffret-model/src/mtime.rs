/// An Entry's modification time, as whole seconds from the Unix epoch.
///
/// Negative values are legal and mean "before 1970" — a file can carry any
/// timestamp its filesystem allows, and rejecting some of them would lose
/// information the Container is supposed to preserve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mtime(i64);

impl Mtime {
    /// Takes a count of seconds from the Unix epoch.
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The count of seconds from the Unix epoch.
    pub const fn as_unix_seconds(&self) -> i64 {
        self.0
    }
}
