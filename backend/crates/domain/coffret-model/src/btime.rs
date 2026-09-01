/// A birth time — when a file was created — as whole seconds from the Unix
/// epoch.
///
/// Optional wherever it appears, because not every platform and filesystem
/// keeps one: an entry table records it when the device that wrote the
/// Container could read it and says nothing at all otherwise. Absent means
/// "never captured", never "created at the epoch".
///
/// A capture-only fact: it is read off the local file at the moment the
/// Container is written and is never stamped onto a fetched file, so once the
/// original file is gone the recorded value is the only one there is.
///
/// Negative values are legal and mean "before 1970", for the reason
/// [`Mtime`](crate::Mtime) admits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Btime(i64);

impl Btime {
    /// Takes a count of seconds from the Unix epoch.
    pub const fn from_unix_seconds(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The count of seconds from the Unix epoch.
    pub const fn as_unix_seconds(&self) -> i64 {
        self.0
    }
}
