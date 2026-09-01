use tokio::sync::{Mutex, MutexGuard};

/// Who is catching the catalog up, which is at most one caller.
///
/// Two at once would take the same starting point out of the catalog
/// (spec: CK-9), read every control object of the Library over again, and then
/// hand the Index the same records from two tasks. The last of those the replay
/// settles on its own, and has to: the other replayer may be a `sync` in another
/// process, which nothing held here reaches — so a record the catalog already
/// holds is stepped over once the checkpoint says somebody applied it, and
/// neither Container ID nor Entry Path is claimed twice (spec: EP-6). What this
/// saves is the rest: a listing walk and a Journal replay spent on Storage to
/// learn what the other one is already learning.
///
/// So the second caller waits, and then asks again rather than being handed the
/// first one's answer. That is the whole difference between this and the
/// per-Entry gate a fetch goes through: an Entry that is present is present, and
/// there is nothing further to find out, while "what is new" is a question about
/// the moment it is asked — a record committed between the two calls is exactly
/// what the second caller is here for, and it costs one listing to find.
#[derive(Debug, Default)]
pub struct Refreshes {
    one_at_a_time: Mutex<()>,
}

impl Refreshes {
    /// Nobody catching up.
    pub fn new() -> Self {
        Self::default()
    }

    /// Waits for whoever is catching up, and takes the turn.
    ///
    /// The turn lasts as long as the returned value, so a caller holds it across
    /// the whole replay and lets go by dropping it — including where the replay
    /// failed, which is what makes a Storage outage something the next caller can
    /// retry rather than something that leaves the gate shut.
    pub(super) async fn turn(&self) -> MutexGuard<'_, ()> {
        self.one_at_a_time.lock().await
    }
}
