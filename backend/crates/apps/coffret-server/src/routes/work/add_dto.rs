use serde::Serialize;

use crate::adds::Add;

/// One add this server is carrying in, as a page is told it (spec: LA-34).
///
/// What a page reads it for is the one thing it cannot hold on to itself: an
/// add it asked to stop and was reloaded before the answer came. The add is
/// listed until its run has ended — or, stopped, until it has been undone —
/// so the page goes on asking, and says it is stopping, until the log says
/// what the stop came to.
#[derive(Serialize)]
pub(super) struct AddDto {
    /// What the add is called: the name its page gave it, or one drawn here.
    id: String,
    /// The folder it was made onto, as a listing spells it.
    folder: String,
    /// `receiving` while its request is still taking files, and `armed` once
    /// the run that carries it in is.
    stage: &'static str,
    /// Whether a person asked for it to be stopped.
    stopping: bool,
}

impl AddDto {
    pub(super) fn of(add: &Add) -> Self {
        Self {
            id: add.id.as_str().to_owned(),
            folder: add.folder.as_str().to_owned(),
            stage: if add.receiving { "receiving" } else { "armed" },
            stopping: add.stopping,
        }
    }
}
