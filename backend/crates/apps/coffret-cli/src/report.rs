//! What a run that succeeded says on standard output, and what it exits with.
//!
//! One summary line, then one line per finding, and nothing else: a person reads
//! the first line and a script reads the exit status, and neither has to parse
//! prose to find out whether the run left work behind.

use coffret_device::{CommitOutcome, Findings};

/// Whether a run that succeeded left anything for somebody to act on.
///
/// The two are the crate's exit statuses `0` and `2`, and every subcommand
/// answers with one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// The run did everything it was asked to.
    Clean,
    /// The run succeeded and left findings.
    Findings,
}

/// What a run says about the generation it committed, in one voice for every
/// command that commits one.
///
/// The committed generation is on a summary because it is the only thing there
/// that says the Library changed: a run with nothing to upload commits nothing,
/// and a Journal record for a batch that changes nothing would be a generation
/// spent on nothing (spec: CP-1).
pub fn committed(commit: Option<&CommitOutcome>) -> String {
    match commit {
        Some(commit) => format!("committed head {}", commit.record.generation.get()),
        None => "committed nothing".to_owned(),
    }
}

/// Prints one line per finding, and says whether any of them is for somebody
/// to act on.
///
/// A settled batch is printed like the rest — it is part of what the run did —
/// but it does not turn the exit status: the run tidied it itself, and a script
/// that stops on `2` must stop for work left behind, not for work done.
pub fn findings(findings: &Findings) -> Report {
    for finding in findings {
        println!("{finding}");
    }
    if findings.needs_attention() {
        Report::Findings
    } else {
        Report::Clean
    }
}
