use crate::folder::Folder;

/// Everything one queue-holding flow has to say about itself at one moment.
///
/// The fill and the freeze both answer this shape, because both keep a queue and
/// both are read the same way: what is on record now, what stopped and is no
/// longer on record, what is waiting, and what the queue lost.
///
/// Read under one borrow, which is the whole reason a flow keeps these in one
/// value. They change together — a worker that dies stops the run and throws the
/// rest of the queue away in one stroke, and a run taken off the queue takes the
/// record from the one before it in another — so a reader that asked four times
/// could carry half of either: a run still saying `filling` beside the folders
/// that very ending threw away, or a run still saying `done` beside a queue
/// already taken up.
#[derive(Clone, Debug)]
pub struct Latest<A> {
    /// The run on record: the one running, or the last one to end.
    pub activity: A,
    /// The runs that stopped and that a later run took the record from, oldest
    /// first.
    ///
    /// Nothing here was displaced mid-run. Each of these had already stopped —
    /// Storage unreachable, or a worker that ended without an answer — when the
    /// next run was taken off the queue and became the one on record. What the
    /// later run took from it is the record and not its turn.
    ///
    /// They are kept because a run that stopped is the one state a person has
    /// something to do about: its folder is sitting outside the Library, its
    /// line is what says so, and the offer of a second attempt hangs off it. A
    /// run that finished has nothing owing, so it goes when the next one starts.
    /// What ends one of these is somebody taking that folder up again.
    pub displaced: Vec<A>,
    /// The folders waiting their turn behind the run on record, oldest first.
    pub waiting: Vec<Folder>,
    /// The folders a worker that ended without an answer threw away, and that
    /// nobody has taken up since.
    pub dropped: Vec<Folder>,
}
