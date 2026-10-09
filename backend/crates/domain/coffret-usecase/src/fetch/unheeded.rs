use crate::fetch::entry_fetch::EntryFetchOutcome;
use crate::fetch::publication::Publication;

/// A fetch whose caller waits for the whole run, and so has nothing to do with
/// the moment its Entry is published.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unheeded;

impl Publication for Unheeded {
    fn published(&self, _outcome: &EntryFetchOutcome) {}
}

/// The one [`Unheeded`] a request defaults to.
pub static UNHEEDED: Unheeded = Unheeded;
