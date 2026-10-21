use super::{DroppedRow, NotTakenRow};

/// What the page that took a drop is told about it: the handle it names in
/// the import, and what each file is.
///
/// Nothing here is a path on this device. Each file is named relative to the
/// folder the dropped item stood in, which is what the browser's own walk calls
/// the same file, and the handle is the only way back to where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    /// The drop handle the import names, or `None` where nothing was taken and
    /// so nothing is held.
    pub handle: Option<String>,
    /// Every file taken, in the order the import names them by: the first is
    /// index 0.
    pub files: Vec<DroppedRow>,
    /// Everything met and not taken.
    pub not_taken: Vec<NotTakenRow>,
}
