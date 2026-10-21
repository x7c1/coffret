use coffret_usecase::drop_walk::{NotTaken, NotTakenReason};

/// One thing a drop met and did not take, as the page sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotTakenRow {
    /// Where it stands relative to the folder the dropped item stood in.
    pub path: String,
    /// Why, in one word: `empty`, `missing`, `not_a_file_or_folder`,
    /// `reserved`, `not_text` or `unreadable`.
    pub reason: &'static str,
}

impl From<NotTaken> for NotTakenRow {
    fn from(not_taken: NotTaken) -> Self {
        Self {
            path: not_taken.path,
            reason: reason_word(not_taken.reason),
        }
    }
}

/// The word one reason travels as.
fn reason_word(reason: NotTakenReason) -> &'static str {
    match reason {
        NotTakenReason::Empty => "empty",
        NotTakenReason::Missing => "missing",
        NotTakenReason::NotAFileOrFolder => "not_a_file_or_folder",
        NotTakenReason::Reserved(_) => "reserved",
        NotTakenReason::NotText => "not_text",
        NotTakenReason::Unreadable => "unreadable",
    }
}
