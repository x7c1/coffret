use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use coffret_device::{ShortKeyring, UnheldParcel};

use crate::finding::Finding;

/// How many findings of reads outside any run are kept before the oldest is
/// dropped.
///
/// What they can be is few — the committed Keyring found short, a kept parcel
/// found not held — and each is said once (see [`ReadFindings::hear`]), so a
/// bound this size is reached only by a device whose kept parcels keep going
/// missing, and there the newest are the ones worth reading.
pub const KEPT_FOUND: usize = 32;

/// What the reads that arm no fill found and would otherwise have said in
/// nobody's line (spec: LA-33).
///
/// A read ahead (`GET /api/file?…&fill=false`) fetches a file where this device
/// does not have it, and that fetch reads the committed Keyring and the parcels
/// the file lies in as any fetch does — so it can find what a fill would have
/// reported: the Keyring short (spec: KL-15), or a kept parcel gone or damaged
/// and read from Storage again (spec: PK-21). A fill carries what its own
/// fetches find in its line; a read ahead has no run to carry it, and handing
/// it to the next fill published would leave it unsaid where no fill follows,
/// as where somebody pages through the covers of a shelf of folders and opens
/// none. So the work answer carries these beside the runs, in its own
/// `findings`.
///
/// Each finding is said once in this process: the same Keyring found short by
/// every cover read is one thing to tell a person, not one per cover. Held in
/// memory alone, like the rest of this process's account of its work, so a
/// server started afterwards holds none.
#[derive(Debug, Default)]
pub struct ReadFindings {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    /// The number the last finding was given, 0 before any: findings are
    /// numbered from 1 in each process.
    next: u64,
    /// The findings, newest at the front.
    found: VecDeque<Found>,
}

/// One finding of a read outside any run, as it was heard.
#[derive(Clone, Debug)]
pub struct Found {
    /// Which finding this is, counted from 1 in this process, so a page can
    /// tell one from another without comparing what they say.
    pub id: u64,
    /// When the server heard it.
    pub at: SystemTime,
    /// What was found, in the shape every run's findings take.
    pub finding: Finding,
}

impl ReadFindings {
    /// Nothing found yet, which is what every process starts with.
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes in what one read's fetch found of the committed Keyring and the
    /// kept parcels it found not held, newest first, each said once.
    ///
    /// A finding already held — the same reason about the same Entry, in the
    /// same words — is not taken again. Past `KEPT_FOUND` the oldest goes.
    pub fn hear(&self, short: Option<ShortKeyring>, unheld: Vec<UnheldParcel>) {
        let findings: Vec<Finding> = short
            .iter()
            .map(coffret_device::Finding::from)
            .chain(unheld.iter().map(coffret_device::Finding::from))
            .filter_map(|found| Finding::of(&found))
            .collect();
        if findings.is_empty() {
            return;
        }
        let at = SystemTime::now();
        let mut inner = self.lock();
        for finding in findings {
            if inner.found.iter().any(|held| same(&held.finding, &finding)) {
                continue;
            }
            inner.next += 1;
            let id = inner.next;
            inner.found.push_front(Found { id, at, finding });
        }
        inner.found.truncate(KEPT_FOUND);
    }

    /// Everything held, newest first.
    pub fn listed(&self) -> Vec<Found> {
        self.lock().found.iter().cloned().collect()
    }

    /// The findings, whatever a writer that panicked left them as: every write
    /// is a few pushes and one truncation, and refusing to answer for the rest
    /// of the process would be worse than a list one finding short.
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Whether two findings say the same thing about the same Entry.
fn same(one: &Finding, other: &Finding) -> bool {
    one.reason == other.reason && one.path == other.path && one.message == other.message
}

#[cfg(test)]
mod tests {
    use coffret_device::{UnheldParcel, UnheldReason};
    use coffret_model::ContainerId;

    use super::ReadFindings;

    fn unheld(container: u8) -> UnheldParcel {
        UnheldParcel {
            container_id: ContainerId::from_bytes([container; ContainerId::BYTE_LEN]),
            index: 0,
            reason: UnheldReason::Missing,
        }
    }

    // The same finding heard twice is said once, and nothing heard is nothing
    // held.
    #[test]
    fn a_finding_heard_again_is_said_once() {
        let found = ReadFindings::new();
        found.hear(None, Vec::new());
        assert!(found.listed().is_empty());

        found.hear(None, vec![unheld(1)]);
        found.hear(None, vec![unheld(2)]);
        let listed = found.listed();
        assert_eq!(listed.len(), 1, "{listed:?}");
        assert_eq!(listed[0].id, 1);
        assert_eq!(listed[0].finding.reason, "parcel_unheld");
    }
}
