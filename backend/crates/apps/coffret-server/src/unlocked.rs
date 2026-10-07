/// What an unlock came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlocked {
    /// The Library was locked and is held open again.
    Now,
    /// It was already open, by an unlock that arrived first; what this one
    /// reopened has been dropped.
    Already,
}
