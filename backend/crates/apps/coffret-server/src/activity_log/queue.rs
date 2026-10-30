/// The two flows that keep a queue of folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
    /// The fill's.
    Fill,
    /// The freeze's.
    Freeze,
}

impl Queue {
    /// The word the queue is told to a page under: its flow's.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Freeze => "freeze",
        }
    }
}
