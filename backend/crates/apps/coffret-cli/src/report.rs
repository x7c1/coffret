//! What a run that succeeded says on standard output, and what it exits with.
//!
//! One summary line, then a line for each Keyring repair the run performed,
//! then one line per finding, and nothing else: a person reads the first line
//! and a script reads the exit status, and neither has to parse prose to find
//! out whether the run left work behind.

use coffret_device::{CommitOutcome, Findings, KeyringRepair};

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
        Some(commit) => format!("committed head {}", commit.record.generation().get()),
        None => "committed nothing".to_owned(),
    }
}

/// What a run says about the committed Keyring generations it repaired — one
/// line each, and no line at all where it repaired nothing (spec: KL-15).
///
/// A line of its own rather than a clause on the summary, because it is not
/// about the run: it is about the Library, and it happens on a run that
/// otherwise did the dullest thing it does. Replica loss is never silent, and
/// the person reading this is the one who can decide whether a Library losing
/// objects is worth looking into.
///
/// One line per repair rather than one per run, because a run that repaired a
/// set and then rebased onto another device's head repaired *that* generation
/// too if it was short, and each line names the generation it is about. More
/// than one line is the rare case; the ordinary repair is one.
///
/// Not a [`Findings`] entry either, for the reason a settled batch is one that
/// does not turn the exit status: this is work the run *did*, and a script that
/// stops on findings must stop for work left behind. A repair that could not
/// complete is not here at all — it refuses the commit, and the run fails with
/// the sentence that refusal renders to.
pub fn repaired(commit: Option<&CommitOutcome>) -> Vec<String> {
    let Some(commit) = commit else {
        return Vec::new();
    };
    commit.repairs.iter().filter_map(repair_line).collect()
}

/// The line one repair renders to, or nothing where it names no position.
///
/// A repair carries at least one rewritten position, so the empty case is only
/// this function refusing to announce a repair that put nothing back.
fn repair_line(repair: &KeyringRepair) -> Option<String> {
    // The words the concept documentation uses, because this is where a person
    // meets them: replicas of a Keyring generation, missing or unreadable, and
    // rewritten from one that survived (spec: KL-6, KL-13).
    let (replicas, was) = match repair.rewritten.len() {
        0 => return None,
        1 => ("1 replica".to_owned(), "was"),
        many => (format!("{many} replicas"), "were"),
    };
    Some(format!(
        "repaired the Keyring: {replicas} of generation {} {was} missing or unreadable, \
         and {was} rewritten from a surviving one",
        repair.generation.get(),
    ))
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
