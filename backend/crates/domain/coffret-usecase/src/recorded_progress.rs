//! The one place this crate's conformance suites read back what a run said
//! about itself while it ran.
//!
//! A [`Progress`] is a callback rather than a terminal, so what a run reports
//! can be asserted on without one — which is the point of the port being a
//! trait and not a print. Every suite that watches a run wants the same thing
//! of it: every step, in the order the run said them. The implementation lives
//! here rather than beside each suite so that two suites cannot come to mean
//! different things by "in order".

use std::sync::Mutex;

use crate::progress::{Progress, Step};

/// A [`Progress`] that keeps every step it was told.
#[derive(Default)]
pub(crate) struct Recording {
    steps: Mutex<Vec<Step>>,
}

impl Recording {
    /// Every step it was told, in the order the run said them.
    pub(crate) fn steps(&self) -> Vec<Step> {
        self.steps
            .lock()
            .expect("nothing here panics holding the lock")
            .clone()
    }
}

impl Progress for Recording {
    fn step(&self, step: Step) {
        self.steps
            .lock()
            .expect("nothing here panics holding the lock")
            .push(step);
    }
}
