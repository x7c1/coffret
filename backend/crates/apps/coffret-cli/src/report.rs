//! What a run that succeeded says on standard output, and what it exits with.
//!
//! One summary line, then a line for each Keyring repair the run performed,
//! then one line per finding, and nothing else: a person reads the first line
//! and a script reads the exit status, and neither has to parse prose to find
//! out whether the run left work behind.
//!
//! A command may put one line of its own under the summary where its counts
//! would otherwise read as an answer they are not — every command that works
//! through the mappings does, on a device that has recorded none. It stays
//! under the summary, before the findings, and it never turns the exit status:
//! nothing went wrong, and the line is there because the numbers above it are
//! true and misleading.

use std::fmt;
use std::num::NonZeroUsize;

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

/// What a run that succeeded and left findings exits with.
const FINDINGS: u8 = 2;

impl Report {
    /// The exit status this answer is.
    ///
    /// Here rather than where the process exits, so that what a report exits
    /// with is asserted on beside what decides the report.
    pub fn exit_status(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::Findings => FINDINGS,
        }
    }
}

/// The line a device that has recorded no mapping gets, and no line otherwise.
///
/// Zeros across a summary are two entirely different answers. A run that found
/// nothing to do is an ordinary empty one — the folders and the Library agree,
/// or the prefix names no current Entry — and reads like one. A device with no
/// mapping at all has nothing in the Library's scope, so it would answer that
/// way about every folder and every prefix there is; and somebody who has just
/// run `join` reads the same zeros as "everything is already here" and stops
/// looking (spec: EP-9).
///
/// One sentence for the three commands that work through the mappings, with
/// one clause apiece for what each of them could not do: a person who tries
/// `sync` first and a person who tries `fetch` first are in the same state and
/// have to be told the same thing, and three wordings of it would read as three
/// different discoveries.
///
/// It is not a finding and does not turn the exit status. Nothing went wrong
/// and the run answered exactly what was asked, so a script that stops on `2`
/// must not stop here; what is missing is a decision nobody has made yet, and
/// the line says which one.
pub fn nothing_mapped(mappings: usize, unmapped: Unmapped) -> Option<String> {
    (mappings == 0).then(|| {
        format!(
            "this device maps no folder, so {unmapped}: record one with `coffret map` and \
             run this again"
        )
    })
}

/// What a device that maps no folder leaves a command unable to do.
///
/// The clause in the middle of the sentence above, and the whole of what
/// differs between the commands: a fetch has nowhere to put what it would
/// bring back, and a sync or a freeze has nothing to carry the other way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmapped {
    /// A fetch: nothing on this device stands for any part of the Library.
    NowhereForTheLibraryToGo,
    /// A sync or a freeze: no local file is inside the Library's scope.
    NothingToCarryIn,
}

impl fmt::Display for Unmapped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self {
            Self::NowhereForTheLibraryToGo => "there is nowhere for the Library to go",
            Self::NothingToCarryIn => "there is nothing of this device to carry into the Library",
        };
        f.write_str(said)
    }
}

/// What a run says about the generation it committed, in one voice for every
/// command that commits one.
///
/// The committed generation is on a summary because it is the only thing there
/// that says the Library changed: a run with nothing to upload commits nothing,
/// and a Journal record for a batch that changes nothing would be a commit
/// slot consumed for nothing (spec: CP-1).
pub fn committed(commit: Option<&CommitOutcome>) -> String {
    committed_line(commit.map(|commit| commit.record.generation().get()))
}

/// The clause itself, over the one thing a commit outcome says here.
///
/// Apart so that what a person reads can be asserted on without a whole commit
/// outcome having to be assembled to say one number.
fn committed_line(generation: Option<u64>) -> String {
    match generation {
        Some(generation) => format!("committed head {generation}"),
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
    commit.repairs.iter().map(repair_line).collect()
}

/// The line one repair renders to.
fn repair_line(repair: &KeyringRepair) -> String {
    repair_sentence(repair.generation.get(), repair.rewritten.count())
}

/// The sentence itself, over the two things a repair says.
///
/// Apart from the repair for the reason [`committed_line`] is apart from the
/// commit outcome: the agreement between the count and the words around it is
/// what regresses, and it is asserted on here rather than through a Library.
fn repair_sentence(generation: u64, rewritten: NonZeroUsize) -> String {
    // The words the concept documentation uses, because this is where a person
    // meets them: replicas of a Keyring generation, missing or unreadable, and
    // rewritten from one that survived (spec: KL-6, KL-13).
    let (replicas, was) = match rewritten.get() {
        1 => ("1 replica".to_owned(), "was"),
        many => (format!("{many} replicas"), "were"),
    };
    format!(
        "repaired the Keyring: {replicas} of generation {generation} {was} missing or \
         unreadable, and {was} rewritten from a surviving one",
    )
}

/// Prints one line per finding, and says whether any of them is for somebody
/// to act on.
///
/// A settled batch is printed like the rest — it is part of what the run did —
/// but it does not turn the exit status: the run tidied it itself, and a script
/// that stops on `2` must stop for work left behind, not for work done. What a
/// commit could not finish after its record — a removal Storage would not
/// trash, a checkpoint not written — is printed the same way and for the same
/// reason: the committed state is correct, and what is left is a later run's.
pub fn findings(findings: &Findings) -> Report {
    let (lines, report) = findings_said(findings);
    for line in lines {
        println!("{line}");
    }
    report
}

/// The lines [`findings`] prints, and the report it answers with.
///
/// Apart from the printing for the reason [`committed_line`] is apart from the
/// commit outcome: which findings turn the exit status is what regresses, and
/// it is asserted on here rather than by reading a process's standard output.
fn findings_said(findings: &Findings) -> (Vec<String>, Report) {
    let lines = findings.iter().map(ToString::to_string).collect();
    let report = if findings.needs_attention() {
        Report::Findings
    } else {
        Report::Clean
    };
    (lines, report)
}

#[cfg(test)]
mod tests {
    use coffret_device::{ContainerId, Disposal, Finding, Generation, Settled, StorageError};

    use super::*;

    /// What a provider that may write and not delete answers a trash with.
    fn trash_refusal() -> StorageError {
        StorageError::PermissionDenied {
            detail: "these credentials may write but not delete".to_owned(),
            source: None,
        }
    }

    // A run whose findings are all things it finished, or left to a later run or
    // for orphan cleanup to find, did everything it was asked to: it prints every
    // one of them and exits 0. A script that stops on `2` must stop for work
    // left behind, and none of these is.
    #[test]
    fn a_run_that_only_settled_and_left_the_commit_to_finish_exits_zero() {
        let container = |seed| ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]);
        let found = Findings::assembled([
            Finding::Settled(Settled::Completed {
                container_id: container(1),
                entries: 1,
            }),
            Finding::Settled(Settled::Disposed {
                container_id: container(2),
                disposal: Disposal::NeverUploaded,
            }),
            Finding::Settled(Settled::Disposed {
                container_id: container(3),
                disposal: Disposal::Trashed,
            }),
            Finding::Settled(Settled::Disposed {
                container_id: container(4),
                disposal: Disposal::LeftInStorage {
                    cause: trash_refusal(),
                },
            }),
            Finding::UntrashedRemoval {
                container_id: container(5),
                cause: trash_refusal(),
            },
            Finding::CheckpointFailed {
                cause: "the provider is unavailable".to_owned(),
            },
        ]);

        let (lines, report) = findings_said(&found);
        assert_eq!(report, Report::Clean);
        assert_eq!(report.exit_status(), 0);
        assert_eq!(lines.len(), 6, "every finding is a line: {lines:?}");
        for (line, seed) in lines.iter().zip(1..=5) {
            assert!(
                line.contains(&container(seed).to_string()),
                "each line names its Container: {line}",
            );
        }
        assert!(lines[0].starts_with("settled container "), "{}", lines[0]);
        assert!(lines[3].contains("still in Storage"), "{}", lines[3]);
        assert!(lines[4].starts_with("untrashed container "), "{}", lines[4]);
        assert!(
            lines[5].starts_with("checkpoint not written "),
            "{}",
            lines[5]
        );
    }

    // The other half of the verdict, so that the one above cannot pass by a
    // report that is always clean: a finding somebody has to act on turns the
    // exit status, whatever else the run tidied.
    #[test]
    fn a_finding_left_for_somebody_exits_two() {
        let container_id = ContainerId::from_bytes([7; ContainerId::BYTE_LEN]);
        let found = Findings::assembled([
            Finding::Settled(Settled::Disposed {
                container_id,
                disposal: Disposal::Trashed,
            }),
            Finding::LockedContainer { container_id },
        ]);

        let (lines, report) = findings_said(&found);
        assert_eq!(report, Report::Findings);
        assert_eq!(report.exit_status(), 2);
        assert_eq!(lines.len(), 2);
    }

    // A run that read a degraded committed Keyring and repaired nothing says
    // so, and exits as it would without it: its reads went on, and the next run
    // that commits repairs the set (spec: KL-15, RV-2). Both ways the finding
    // is said — a loss the read established, and replicas Storage merely did
    // not hand over.
    #[test]
    fn a_degraded_keyring_is_said_and_exits_zero() {
        let found = Findings::assembled([
            Finding::DegradedKeyring {
                generation: Generation::FIRST,
                replicas: 3,
                lost: 1,
                unfetched: 0,
            },
            Finding::DegradedKeyring {
                generation: Generation::FIRST,
                replicas: 3,
                lost: 0,
                unfetched: 1,
            },
        ]);

        let (lines, report) = findings_said(&found);
        assert_eq!(report, Report::Clean);
        assert_eq!(report.exit_status(), 0);
        assert!(lines[0].starts_with("degraded keyring: "), "{}", lines[0]);
        assert!(!lines[1].contains("degraded keyring"), "{}", lines[1]);
    }

    // A run that committed nothing says so rather than saying nothing: the line
    // is what tells a person the Library is unchanged (spec: CP-1).
    #[test]
    fn a_run_that_committed_nothing_says_so() {
        assert_eq!(committed_line(None), "committed nothing");
    }

    #[test]
    fn a_run_that_committed_names_the_head_it_left() {
        assert_eq!(committed_line(Some(7)), "committed head 7");
    }

    #[test]
    fn one_position_is_said_in_the_singular() {
        assert_eq!(
            repair_sentence(4, NonZeroUsize::MIN),
            "repaired the Keyring: 1 replica of generation 4 was missing or unreadable, \
             and was rewritten from a surviving one",
        );
    }

    #[test]
    fn more_than_one_position_is_said_in_the_plural() {
        assert_eq!(
            repair_sentence(4, NonZeroUsize::new(3).expect("three is not zero")),
            "repaired the Keyring: 3 replicas of generation 4 were missing or unreadable, \
             and were rewritten from a surviving one",
        );
    }

    // A run with no commit repaired nothing, which is no line rather than an
    // empty one.
    #[test]
    fn a_run_that_did_not_commit_reports_no_repair() {
        assert!(repaired(None).is_empty());
    }

    // A device that maps something is in no such state, whatever its counts
    // came to: the line is about a device with none.
    #[test]
    fn a_device_that_maps_something_is_told_nothing() {
        for unmapped in [
            Unmapped::NowhereForTheLibraryToGo,
            Unmapped::NothingToCarryIn,
        ] {
            assert_eq!(nothing_mapped(1, unmapped), None);
        }
    }

    // The three commands say one thing in one wording: the same state, the
    // same gesture out of it, and one clause apiece for what each could not
    // do. Three discoveries of the same fact would read as three problems.
    #[test]
    fn every_command_says_the_same_thing_about_a_device_that_maps_nothing() {
        let fetch = nothing_mapped(0, Unmapped::NowhereForTheLibraryToGo)
            .expect("a device that maps nothing is worth a line");
        let carrying = nothing_mapped(0, Unmapped::NothingToCarryIn)
            .expect("a device that maps nothing is worth a line");

        for said in [&fetch, &carrying] {
            assert!(
                said.contains("maps no folder"),
                "it must say what the state is: {said:?}",
            );
            assert!(
                said.contains("`coffret map`"),
                "and what leaves it: {said:?}",
            );
        }
        assert!(
            fetch.contains("nowhere for the Library to go")
                && carrying.contains("nothing of this device to carry"),
            "and each says what its own command could not do: {fetch:?}, {carrying:?}",
        );
    }
}
