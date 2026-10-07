//! Telling whoever started a run how far into it it is.
//!
//! A sync or a fetch over a folder of any size spends minutes inside one call,
//! and nothing about that call says so: it returns an outcome or an error, and
//! until it does there is no difference a person can see between a transfer
//! that is working and one that is stuck. The consent step already solved the
//! same problem the same way — it is handed a callback and says a word while it
//! waits — and this is that shape for the part that takes the time.
//!
//! It is a capability rather than anything the flow decides, which is why it
//! sits beside the ports: a use case knows how many Containers it is going to
//! read and how many files it has packed, and nothing about what it does with
//! them depends on who is watching. A caller that wants no report passes
//! [`Unwatched`] and the calls go nowhere, so the flow never asks whether
//! anybody is listening.

/// What a run is doing while it is doing it.
///
/// Only the phases a caller could tell apart from the outside are here, and
/// they are in the order a run meets them. Naming every internal step would be
/// a vocabulary that changes whenever the flow is rearranged; what earns a name
/// is a stretch of time a person waiting could otherwise mistake for a run that
/// has stopped.
///
/// The first three of them cannot say how much work they hold — what the
/// catch-up has to replay is known only as it is read, and the scan is the very
/// thing that counts the files — which is why a [`Step`] may carry no total.
/// They are reported all the same, because a phase that cannot count what it
/// will do is exactly the phase that goes quiet for minutes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Bringing this device's catalog up to the Library's head (spec: CK-9).
    ///
    /// Every flow here begins with it, and on a device that has just joined it
    /// replays the whole Journal before anything else happens.
    CatchingUp,
    /// Settling the pending rows an interrupted run left behind
    /// (spec: OC-2, OC-3, OC-7).
    ///
    /// Said only by a run that has rows to settle. The ordinary run has none,
    /// and one that said this anyway would tell a person about an interruption
    /// that never happened.
    Settling,
    /// Reading this device's mapped folders and deciding what the run will
    /// work on (spec: EP-9, EP-10).
    Scanning,
    /// Encoding local files into Containers on this device.
    ///
    /// The unit is a file for a sync and a Pack for a freeze, because that is
    /// what each of them cuts its work into.
    Packing,
    /// Sending encoded Containers to Storage.
    Uploading,
    /// Reading Containers back from Storage and placing the files in them.
    Fetching,
}

/// How far into one phase a run has got.
///
/// A count of things done out of things to do: what a person watching a
/// transfer wants to know is whether it is moving and roughly how much is left,
/// and the unit that answers that is the one the run loops over.
///
/// `done` is what has finished rather than what has started, so a step is
/// reported with `done` at zero before the first unit of work and with `done`
/// equal to `total` when the phase is over.
///
/// `total` is absent where the phase cannot say how much work it holds — a
/// catch-up learns what it has to replay by replaying it, and a scan is the
/// thing that counts the files. Such a phase says once that it has begun and
/// nothing after that, which is all a caller can render: the phase's name,
/// without numbers. It is not the same state as a phase with nothing to do,
/// and the two must not be shown alike — see [`Step::begun`].
///
/// A phase a run never enters is never reported, and one it enters with nothing
/// to do may be reported as `0` of `0`. A caller has nothing to show for the
/// latter; what a run did is in its outcome, and this says only that it is
/// still going.
///
/// # Bytes, for the one phase whose unit can be large
///
/// A step of [`Phase::Uploading`] also carries [`bytes`](Self::bytes): how many
/// bytes of the whole phase Storage has taken, out of how many the phase sends.
/// Every other phase carries none, and a step of this one always carries it.
///
/// The unit count alone holds for a phase made of many small units, where the
/// count moves often enough to say the run is moving. It fails for the upload of
/// a freeze, whose unit is a Pack of tens of megabytes: a book is often one
/// Pack, so the count reads `0/1` for as long as that Pack takes to send, and a
/// person cannot tell a slow upload from a stuck one. The bytes are what moves
/// while the count cannot. Packing is not given them, because it runs on this
/// device at the speed of its own disk; fetching is not, yet, because nothing
/// there has been seen to stand still.
///
/// The bytes are of the whole phase rather than of the unit in flight, because
/// that is the number that reads alongside the count without a second rule:
/// `done` says how many units have finished and the bytes say how much of all of
/// them has gone, so neither one has to say which unit the other is about, and
/// the total is known before the first byte moves and does not change. Between
/// the reports at unit boundaries a run reports them as the bytes go, at most a
/// few times a second.
///
/// What is counted is what Storage has pulled from the stream it was handed —
/// the port takes a body as a stream (see [`ObjectStore::put`]) — which is what
/// has gone out only for a store that sends as it pulls. A put that fails and
/// is tried again starts its object from the first byte, and so do the bytes:
/// what they count is what is under way, and a count that never went back would
/// say an attempt had sent what it had not.
///
/// [`ObjectStore::put`]: crate::ObjectStore::put
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    /// Which phase the run is in.
    pub phase: Phase,
    /// How many units of that phase have finished.
    pub done: usize,
    /// How many there are in all, where the phase can say.
    pub total: Option<usize>,
    /// How many bytes of the phase Storage has taken, for a phase that counts
    /// them — which is [`Phase::Uploading`] and no other.
    pub bytes: Option<ByteCount>,
}

/// How many bytes of a phase have gone, out of how many it sends.
///
/// Its own type rather than two more fields on [`Step`], because the two go
/// together or not at all: a phase that counts bytes knows its total before the
/// first one moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteCount {
    /// How many bytes have gone.
    pub done: u64,
    /// How many the phase sends in all.
    pub total: u64,
}

impl Step {
    /// A step of `phase` with `done` of `total` finished.
    pub const fn new(phase: Phase, done: usize, total: usize) -> Self {
        Self {
            phase,
            done,
            total: Some(total),
            bytes: None,
        }
    }

    /// The same step, saying how many bytes of the phase have gone.
    pub const fn with_bytes(self, bytes: ByteCount) -> Self {
        Self {
            bytes: Some(bytes),
            ..self
        }
    }

    /// A step saying `phase` has begun, by a phase that cannot count its work.
    ///
    /// Distinct from `0` of `0`, which says the run passed through a phase with
    /// nothing in it: there a caller has nothing to show, and here it has the
    /// one thing the person waiting wants — that the run is inside a phase and
    /// has not stopped.
    pub const fn begun(phase: Phase) -> Self {
        Self {
            phase,
            done: 0,
            total: None,
            bytes: None,
        }
    }
}

/// Where a run says what it is doing.
///
/// Implemented by the shell that started the run — a terminal renderer for the
/// command line, a field of the work answer a browser polls for the server — and
/// never by anything below it: this layer decides what is worth reporting and
/// the caller decides what to do with it.
///
/// Every call is made from inside the flow, so an implementation must not
/// block, fail, or panic: it is a report and not a request, and a run that a
/// progress line could stop would be a run whose correctness depended on who
/// was watching it.
pub trait Progress: Send + Sync {
    /// Says that the run has reached `step`.
    fn step(&self, step: Step);
}

/// The progress of a run nobody is watching.
///
/// What every request defaults to, and what a shell with nowhere to report
/// passes. It is a type rather than an `Option` so that a flow reports
/// unconditionally: an `if let Some(..)` at every reporting point is a branch
/// on who the caller is, and the one that is forgotten is the one that goes
/// quiet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unwatched;

impl Progress for Unwatched {
    fn step(&self, _step: Step) {}
}

/// The [`Unwatched`] every request starts from.
///
/// A borrow of a value with no state needs somewhere to borrow from, and one
/// static serves every request there will ever be.
pub static UNWATCHED: Unwatched = Unwatched;

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// A progress that keeps what it was told.
    #[derive(Default)]
    struct Recording {
        steps: Mutex<Vec<Step>>,
    }

    impl Progress for Recording {
        fn step(&self, step: Step) {
            self.steps.lock().expect("no case poisons this").push(step);
        }
    }

    // The trait is used as `&dyn Progress` everywhere a request holds one, so
    // it has to stay object safe — and a shared reference to one has to be
    // usable from the futures the flows build, which are `Send`.
    #[test]
    fn a_progress_is_reported_to_through_a_shared_reference() {
        let recording = Recording::default();
        let progress: &dyn Progress = &recording;
        fn assert_send<T: Send>(_value: &T) {}
        assert_send(&progress);

        progress.step(Step::new(Phase::Uploading, 1, 3));
        assert_eq!(
            recording
                .steps
                .lock()
                .expect("no case poisons this")
                .as_slice(),
            [Step::new(Phase::Uploading, 1, 3)],
        );
    }

    // Nothing is kept and nothing is refused: the calls a run makes into it
    // simply end there.
    #[test]
    fn an_unwatched_run_reports_into_nothing() {
        UNWATCHED.step(Step::new(Phase::Packing, 0, 0));
        UNWATCHED.step(Step::begun(Phase::CatchingUp));
    }

    // The two empty-looking states are different states: a phase that has
    // begun and cannot say how much it holds, and a phase the run passed
    // through with nothing to do. A caller shows the first and shows nothing
    // for the second, so they must not compare equal.
    #[test]
    fn a_phase_that_has_begun_is_not_a_phase_with_nothing_in_it() {
        let begun = Step::begun(Phase::CatchingUp);
        assert_eq!(begun.total, None);
        assert_ne!(begun, Step::new(Phase::CatchingUp, 0, 0));
    }
}
