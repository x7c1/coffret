/// One file a drop took, as the page sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedRow {
    /// Its path relative to the folder the dropped item stood in: `photo.jpg`
    /// for a file dropped on its own, `holiday/day1/photo.jpg` for one inside
    /// a dropped folder.
    pub path: String,
    /// Its length when the walk stated it.
    pub size: u64,
    /// Its modification time, in whole milliseconds from the Unix epoch — what
    /// a browser calls a file's `lastModified`.
    pub mtime_millis: i64,
}
