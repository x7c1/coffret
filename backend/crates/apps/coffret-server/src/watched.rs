use coffret_device::{Progress, Step};

/// Where a flow's progress goes when what is watching it is a browser.
///
/// The command line's answer to "a run that takes minutes says nothing" is a
/// line rewritten in place, and the flows report to it through
/// [`Progress`](coffret_device::Progress) — a capability passed in from the
/// shell, beside the ones for asking a Passphrase and opening a browser. This
/// is the same port with the other shell behind it: what a step becomes here is
/// a field of the activity a browser polls, so the two shells show one run's
/// progress from one source rather than each counting for itself.
///
/// A closure rather than a type per flow, because what each of them does with a
/// step is one line — write it onto the activity on record — and three structs
/// to say that would be three places for the three to drift apart.
///
/// What the port asks of an implementation is that it never blocks, fails or
/// panics: it is a report and not a request. Writing one field under the watch
/// channel's lock is the whole of it, and the lock is held by nothing that
/// awaits.
pub(crate) struct Watched<F>(F)
where
    F: Fn(Step) + Send + Sync;

impl<F> Watched<F>
where
    F: Fn(Step) + Send + Sync,
{
    /// Reports every step of a run to `record`.
    pub(crate) fn by(record: F) -> Self {
        Self(record)
    }
}

impl<F> Progress for Watched<F>
where
    F: Fn(Step) + Send + Sync,
{
    fn step(&self, step: Step) {
        (self.0)(step);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use coffret_device::{Phase, Progress, Step};

    use super::Watched;

    // The whole of what this type is: a step reported by a flow reaches the
    // line that records it, unchanged. A shared reference is what a request
    // holds one by, so that is what it is exercised through.
    #[test]
    fn a_step_a_flow_reports_reaches_whoever_records_it() {
        let recorded = Mutex::new(Vec::new());
        let watched = Watched::by(|step| {
            recorded.lock().expect("no case poisons this").push(step);
        });

        let progress: &dyn Progress = &watched;
        progress.step(Step::begun(Phase::Scanning));
        progress.step(Step::new(Phase::Packing, 2, 5));

        assert_eq!(
            recorded.lock().expect("no case poisons this").as_slice(),
            [
                Step::begun(Phase::Scanning),
                Step::new(Phase::Packing, 2, 5)
            ],
        );
    }
}
