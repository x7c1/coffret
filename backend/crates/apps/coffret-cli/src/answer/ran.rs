//! What a command that succeeded hands back.

use coffret_device::Findings;

use crate::report::Report;

use super::{Answer, Found};

/// What a command that succeeded hands back to be answered with.
///
/// The text form has been printed by the time one of these exists, since each
/// command prints its own; this is what the JSON form is made of.
pub struct Ran {
    /// Whether the run left anything for somebody to act on.
    pub report: Report,
    /// What the command answered.
    pub answer: Answer,
    /// The findings the run reported, in the order the text form prints them.
    pub findings: Vec<Found>,
}

impl Ran {
    /// A run that answered and has no findings to report.
    pub fn clean(answer: Answer) -> Self {
        Self {
            report: Report::Clean,
            answer,
            findings: Vec::new(),
        }
    }

    /// A run that answered and reported `findings`, exiting as `report` says.
    pub fn found(report: Report, answer: Answer, findings: &Findings) -> Self {
        Self {
            report,
            answer,
            findings: findings.iter().map(Found::from).collect(),
        }
    }
}
