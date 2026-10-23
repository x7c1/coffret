use std::sync::atomic::{AtomicBool, Ordering};

use super::ShortKeyring;

/// A read's finding, held for as long as the run that made it may still speak
/// for it, and said on the way out where nothing did.
///
/// What a flow that reads the committed Keyring and then goes on to write
/// needs, and the reason it is a guard rather than a line at the end: a run
/// leaves by every `?` it contains, and a set found short is worth the same one
/// word whether the run committed, found nothing to commit, or died halfway. So
/// the finding is said when this is dropped, from wherever the run ended.
///
/// The one path that stays silent is the one that has already spoken.
/// [`examine`](super::examine) walks the same committed generation exhaustively and says what
/// it found and what it put back (spec: KL-11, KL-15), so a commit whose
/// examination has spoken disarms this through [`examined`](Self::examined) —
/// one set, one run, one line. An examination that failed before it said
/// anything leaves this armed, so that its drop speaks instead.
///
/// Usually the read and the examination walk one generation. Where they do
/// not, the silence is still the right answer: a commit catches the Index up
/// before it examines anything, so a generation another device committed
/// while this run was packing is the one the examination walks, and the set
/// the read found short is by then not the committed set at all. KL-5 is a
/// statement about the committed set, and what became of a superseded
/// generation's replicas is orphan cleanup's question (spec: CK-9, KL-12).
pub(crate) struct ShortReport {
    /// The finding, where the read made one.
    found: Option<ShortKeyring>,
    /// Whether something else in the run has spoken for it.
    ///
    /// An atomic rather than a `Cell` because the disarming happens inside the
    /// commit, across its awaits, and a future that holds this has to stay
    /// `Send`.
    spoken_for: AtomicBool,
}

impl ShortReport {
    /// The finding a read left, waiting to be said.
    pub(crate) fn armed(found: Option<ShortKeyring>) -> Self {
        Self {
            found,
            spoken_for: AtomicBool::new(false),
        }
    }

    /// Hands the finding over to the examination this run has made.
    ///
    /// Called once a commit's examination of the committed set has spoken:
    /// that walk is the exhaustive one, and what it reports is the fuller
    /// account of the very thing the read noticed (spec: KL-15).
    pub(crate) fn examined(&self) {
        self.spoken_for.store(true, Ordering::Relaxed);
    }

    /// The finding, where nothing else in the run has spoken for it.
    ///
    /// What a run that ends without an examination hands its caller on its
    /// outcome, beside the line its drop writes: the two are the same word on
    /// two channels, one for the person who ran the flow and one for the log.
    /// Where the commit's examination spoke, its own account travels instead
    /// (the repair on the commit outcome, or the refusal), and this is `None`.
    pub(crate) fn unspoken(&self) -> Option<ShortKeyring> {
        if self.spoken_for.load(Ordering::Relaxed) {
            return None;
        }
        self.found
    }
}

impl Drop for ShortReport {
    fn drop(&mut self) {
        if let Some(found) = self.unspoken() {
            found.report();
        }
    }
}
