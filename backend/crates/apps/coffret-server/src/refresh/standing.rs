use std::sync::RwLock;

use crate::reported::Reported;

/// How this device's catalog stands with the Library.
///
/// Three states and the whole set is named here, because a browser writes a
/// branch per state and one it has never heard of is one it falls off the end
/// of.
///
/// They exist because of one sentence a screen would otherwise have to invent:
/// an explorer over an empty listing cannot tell a Library with nothing in it
/// from a device that has not learnt what is in it. The catalog is what every
/// listing comes out of, and it is a cache of what this device has replayed
/// (spec: CK-9) — so a device fresh from `join` whose catch-up did not land
/// lists nothing, and lists nothing in exactly the way a genuinely empty
/// Library does.
#[derive(Clone, Debug)]
pub enum Standing {
    /// A catch-up is running right now, and what the catalog holds is what it
    /// held before it started.
    ///
    /// This is where a process begins: the startup catch-up runs before the
    /// socket is bound, so the first thing a browser is told is the outcome of
    /// that one rather than this — but a refresh somebody pressed puts the
    /// catalog back here while it runs, and a second tab asking meanwhile is
    /// owed the true answer rather than the last one.
    CatchingUp,
    /// The last catch-up finished, so the catalog stood at the Library's head
    /// when it did.
    ///
    /// Not a claim about this instant. Another device may commit a moment
    /// later, and nothing here follows the remote head — what this says is that
    /// this device has replayed what the Library had, which is the difference
    /// between an empty listing that is an answer and one that is a gap.
    CaughtUp,
    /// The last catch-up did not finish, and this says what stopped it.
    ///
    /// The listing is still served, deliberately: reading what the Index holds
    /// needs no Storage, and the offline half of the explorer is meant to work.
    /// What must not happen is that half being shown as though it were the
    /// whole.
    Behind(Reported),
}

/// How the catalog stands, as the one value every reader takes it from.
///
/// A lock rather than a [`watch`](tokio::sync::watch) channel, unlike the three
/// run-tracking values beside it: nobody waits on this. It is written at the
/// two moments a catch-up begins and ends, and read once per activity request.
#[derive(Debug)]
pub struct Catalog {
    standing: RwLock<Standing>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

impl Catalog {
    /// A catalog nobody has caught up yet.
    ///
    /// [`CatchingUp`](Standing::CatchingUp) and not
    /// [`CaughtUp`](Standing::CaughtUp), because the startup catch-up is the
    /// next thing a process does and a value that claimed otherwise until it
    /// finished would be the lie this type exists to prevent.
    pub fn new() -> Self {
        Self {
            standing: RwLock::new(Standing::CatchingUp),
        }
    }

    /// How it stands.
    pub(crate) fn standing(&self) -> Standing {
        self.read().clone()
    }

    /// Records that a catch-up has begun.
    pub(super) fn catching_up(&self) {
        self.write(Standing::CatchingUp);
    }

    /// Records that one finished.
    pub(super) fn caught_up(&self) {
        self.write(Standing::CaughtUp);
    }

    /// Records that one did not, and what stopped it.
    pub(super) fn behind(&self, trouble: Reported) {
        self.write(Standing::Behind(trouble));
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Standing> {
        // A poisoned lock here would be a panic while one of the four lines
        // above was running, and every one of them is an assignment. Reading
        // through it is not a risk taken: what it protects is a value with no
        // invariant between its parts.
        self.standing
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self, standing: Standing) {
        *self
            .standing
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = standing;
    }
}

#[cfg(test)]
mod tests {
    use super::{Catalog, Standing};
    use crate::reported::Reported;

    // Where a process begins, and the reason it begins there: the startup
    // catch-up has not run, so a catalog claiming to be current would be
    // claiming it of a device that has replayed nothing at all.
    #[test]
    fn a_catalog_starts_out_not_having_caught_up() {
        assert!(matches!(Catalog::new().standing(), Standing::CatchingUp));
    }

    #[test]
    fn a_catch_up_that_did_not_finish_keeps_what_stopped_it() {
        let catalog = Catalog::new();
        catalog.caught_up();
        catalog.behind(Reported::gave_up());

        let Standing::Behind(trouble) = catalog.standing() else {
            panic!("a catch-up that did not finish leaves the catalog behind");
        };
        assert_eq!(trouble.kind, "storage");

        catalog.caught_up();
        assert!(
            matches!(catalog.standing(), Standing::CaughtUp),
            "and the next one that lands takes the trouble off the screen",
        );
    }
}
