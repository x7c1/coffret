use std::time::SystemTime;

use super::Ended;

/// One finished piece of work, as the log keeps it.
#[derive(Clone, Debug)]
pub struct Logged {
    /// Which entry this is, counted from 1 in this process, so a page can tell
    /// one entry from another without comparing what they say.
    pub id: u64,
    /// When the server logged it.
    pub at: SystemTime,
    /// What ended.
    pub ended: Ended,
    /// Whether a later run of the same work — for a refused file, the same
    /// file asked for again — has taken this one's place.
    ///
    /// Meaningful for a failure alone: a failure stays standing — kept past
    /// `KEPT`, and offered a second attempt — until the same work ends again,
    /// whichever way, and then it is history like any other entry.
    pub settled: bool,
}

impl Logged {
    /// Whether this is a failure nothing has settled yet: the entries a page
    /// counts as failed, and the ones the log keeps past `KEPT`.
    pub fn standing(&self) -> bool {
        !self.settled && self.ended.failed()
    }
}
