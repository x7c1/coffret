use super::{Declined, FillStatus, Folder};
use crate::reported::Reported;

/// What one fill of one folder has come to.
///
/// The server's own state and not the Library's: it is about work in flight on
/// this device, it is gone when the process is, and none of it is ever
/// uploaded. The catalog keeps saying `present` or `remote` about every Entry
/// throughout (spec: EP-10) — this says which of the `remote` ones something is
/// doing something about right now.
#[derive(Clone, Debug)]
pub struct Activity {
    /// The folder being brought over.
    pub folder: Folder,
    /// Where the fill stands.
    pub status: FillStatus,
    /// How many of the folder's files the fill set out to bring over — the
    /// `remote` rows of the listing it started from, and `0` until it has read
    /// that listing.
    pub total: usize,
    /// How many of them are on this device now.
    pub done: usize,
    /// The Entries the fill did not bring over, and what it found instead.
    pub declined: Vec<Declined>,
    /// The refusal that stopped the fill, where one did.
    ///
    /// One refusal and not one per Entry: what stops a fill is Storage being
    /// unreachable or the grant having run out, and every Entry left in the
    /// folder would have met it identically.
    pub stopped: Option<Reported>,
}

impl Activity {
    /// A fill that has been armed and has not read its folder's listing yet.
    pub(super) fn starting(folder: Folder) -> Self {
        Self {
            folder,
            status: FillStatus::Filling,
            total: 0,
            done: 0,
            declined: Vec::new(),
            stopped: None,
        }
    }
}
