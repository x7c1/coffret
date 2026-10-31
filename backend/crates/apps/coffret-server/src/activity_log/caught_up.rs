use coffret_device::CatchUpOutcome;

/// What a catch-up that landed came to: the three numbers a person who asked
/// what is new is answered with (`POST /api/refresh`).
///
/// Kept apart from [`CatchUpOutcome`] because the log keeps it for as long as
/// the process runs, and these three are all a page words it with: the
/// generations the catalog moved between are the Library's own bookkeeping,
/// which the server's log already carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaughtUp {
    /// Whether the Library had a head this device had not seen.
    pub advanced: bool,
    /// How many current Entries the catalog gained — negative where another
    /// device's commit removed more than it added.
    pub gained: i64,
    /// How many current Entries the Library holds now.
    pub entries: usize,
}

impl CaughtUp {
    /// What `outcome` came to.
    pub fn of(outcome: &CatchUpOutcome) -> Self {
        Self {
            advanced: outcome.advanced(),
            gained: outcome.gained(),
            entries: outcome.entries_after,
        }
    }
}
