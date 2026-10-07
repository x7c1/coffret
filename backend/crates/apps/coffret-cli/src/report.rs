//! What a run says on standard output and standard error, and what it exits
//! with.
//!
//! A run that succeeded says one summary line, then one line per finding —
//! each Keyring repair the run performed among them, after the findings about
//! what it was asked to do — and nothing else: a person reads the first line
//! and a script reads the exit status, and neither has to parse prose to find
//! out whether the run left work behind. A repair is a finding nobody has to
//! act on, so it never turns the exit status.
//!
//! A run that failed says the Keyring repairs it performed before it failed,
//! in the same words and on the same stream as a run that succeeded, and then
//! the failure itself on standard error: the chain of what each layer
//! reported, and after it what a person can do about it, if anything.
//!
//! A command may put one line of its own under the summary where its counts
//! would otherwise read as an answer they are not — every command that works
//! through the mappings does, on a device that has recorded none. It stays
//! under the summary, before the findings, and it never turns the exit status:
//! nothing went wrong, and the line is there because the numbers above it are
//! true and misleading.

use std::error;
use std::fmt;

use coffret_device::{CommitFailure, CommitOutcome, Error, Findings};

use crate::answer::Form;

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
    /// What a run that failed exits with, which no report is: a run that
    /// failed has no answer to report on.
    pub const FAILED: u8 = 1;

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

/// What a run that failed says, in the order it is printed.
#[derive(Debug, PartialEq, Eq)]
pub struct Failed {
    /// One line per Keyring repair the run performed before it failed, for
    /// standard output, as [`findings`] says them on a run that succeeded.
    pub repaired: Vec<String>,
    /// The failure, for standard error: the whole chain on one line, then each
    /// piece of advice on a line of its own.
    pub said: Vec<String>,
}

/// What a run that failed with `error` says (spec: KL-15).
///
/// The repairs first, because they are not about the failure: the replicas a
/// commit put back stand on Storage whatever became of the batch, and a repair
/// performed is never silent. They are the ones a failed commit carries, found
/// wherever in the chain the commit's failure is, and said as the device's
/// findings say them — the sentence the explorer shows for the same repair.
///
/// Then the chain: what failed, and under it what each layer reported, down to
/// the format crate's or the provider's own words. Then advice, after all of
/// it. A layer's sentence is printed before its cause's, so advice spoken in
/// one would reach a person before the reason for it; an error that has
/// something to advise says it apart, and it is printed last, on its own line,
/// where it follows the cause rather than preceding it. It is for the person at
/// the terminal: the log carries the refusal's redacted form, which has none.
pub fn failed(error: &anyhow::Error) -> Failed {
    let links = || error.chain();
    let repaired = Findings::repaired_before(error.as_ref())
        .iter()
        .map(ToString::to_string)
        .collect();
    let mut said = vec![format!("{error:#}")];
    said.extend(links().filter_map(advice).map(str::to_owned));
    Failed { repaired, said }
}

/// What one link of a chain advises, where it is an error that can.
///
/// The errors that advise and have a source are the ones asked here: a stopped
/// Keyring repair, which only the commit flow's examination raises and so
/// always arrives on a [`CommitFailure`], and a Library whose app folder may
/// have been created before the answer was lost. An error with no source says
/// what to do in its own sentence, which is already the last line of the chain.
fn advice(link: &(dyn error::Error + 'static)) -> Option<&'static str> {
    if let Some(failure) = link.downcast_ref::<CommitFailure>() {
        return failure.advice();
    }
    link.downcast_ref::<Error>().and_then(Error::advice)
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
///
/// Under `--json` nothing is printed: the findings are in the answer instead.
pub fn findings(findings: &Findings, form: Form) -> Report {
    let (lines, report) = findings_said(findings);
    if form.is_text() {
        for line in lines {
            println!("{line}");
        }
    }
    report
}

/// Prints what a person reads on standard output to know what a run did.
///
/// Under `--json` the first line — the counts — is in the answer instead, and
/// any line after it is advice for a person, which goes to standard error
/// where the rest of the advice is, so that standard output holds the answer
/// and nothing else.
pub fn summary(lines: &[String], form: Form) {
    match form {
        Form::Text => {
            for line in lines {
                println!("{line}");
            }
        }
        Form::Json => {
            for line in lines.iter().skip(1) {
                eprintln!("{line}");
            }
        }
    }
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
    use coffret_device::{
        CommitError, ContainerId, CreationStep, Disposal, Finding, Generation, KeyringRepair,
        RewrittenReplicas, Settled, StorageError, SyncError, UnrepairedReplica,
    };

    use super::*;

    /// A sync that failed in its commit with `error`, after the commit had
    /// performed `repairs`, as the command line receives it.
    fn a_failed_sync(error: CommitError, repairs: Vec<KeyringRepair>) -> anyhow::Error {
        Error::from(SyncError::Commit(CommitFailure {
            error: Box::new(error),
            repairs,
        }))
        .into()
    }

    /// A repair of `generation` that put back `positions`.
    fn repair(generation: u64, positions: Vec<u16>) -> KeyringRepair {
        KeyringRepair {
            generation: Generation::new(generation).expect("a representable generation"),
            rewritten: RewrittenReplicas::assembled(positions)
                .expect("a repair puts back at least one position"),
        }
    }

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
    fn an_uncertain_commit_is_reported_as_retained_and_needs_attention() {
        let found = Findings::assembled([Finding::Settled(Settled::Retained {
            container_id: ContainerId::from_bytes([9; ContainerId::BYTE_LEN]),
        })]);
        let (lines, report) = findings_said(&found);
        assert_eq!(report.exit_status(), 2);
        assert!(lines[0].contains("retained container"));
        assert!(lines[0].contains("commit outcome is unknown"));
    }

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

    // A run that repaired the Keyring says each repair, in the singular and the
    // plural, and exits as it would without them: the set is whole again, and
    // a script that stops on `2` must stop for work left behind (spec: KL-15).
    #[test]
    fn a_repair_is_said_and_exits_zero() {
        let found = Findings::assembled([
            Finding::from(&repair(4, vec![1])),
            Finding::from(&repair(4, vec![0, 1, 2])),
        ]);

        let (lines, report) = findings_said(&found);
        assert_eq!(report, Report::Clean);
        assert_eq!(
            lines,
            [
                "repaired the Keyring: 1 replica of generation 4 was missing or unreadable, \
                 and was rewritten from a surviving one",
                "repaired the Keyring: 3 replicas of generation 4 were missing or unreadable, \
                 and were rewritten from a surviving one",
            ],
        );
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

    // KL-15: a run that repaired the Keyring and then failed still says the
    // repair, in the words a run that committed says it in — the replicas it
    // put back stand whatever became of the batch.
    #[test]
    fn a_run_that_failed_after_a_repair_says_the_repair() {
        let failed = failed(&a_failed_sync(
            CommitError::ConflictLimitReached { attempts: 8 },
            vec![repair(4, vec![1]), repair(5, vec![0, 2])],
        ));

        assert_eq!(
            failed.repaired,
            [
                "repaired the Keyring: 1 replica of generation 4 was missing or unreadable, \
                 and was rewritten from a surviving one",
                "repaired the Keyring: 2 replicas of generation 5 were missing or unreadable, \
                 and were rewritten from a surviving one",
            ],
        );
        assert_eq!(
            failed.said,
            [
                "the sync did not finish: the sync did not come through the commit flow: the \
                 commit slot was taken by another writer on all 8 attempts"
            ],
            "a failure with nothing to advise is the chain alone",
        );
    }

    // A failure that repaired nothing says nothing about the Keyring, which is
    // no line rather than an empty one.
    #[test]
    fn a_run_that_failed_without_a_repair_says_none() {
        let failed = failed(&a_failed_sync(
            CommitError::ConflictLimitReached { attempts: 8 },
            Vec::new(),
        ));
        assert!(failed.repaired.is_empty(), "{failed:?}");
    }

    // KL-16: a refusal that advises is read cause first — the chain down to
    // what Storage said — and what to do about it after, on its own line.
    #[test]
    fn a_stopped_repair_says_what_to_do_after_the_cause() {
        let failed = failed(&a_failed_sync(
            CommitError::UnrepairedKeyring {
                generation: Generation::FIRST,
                needed: vec![1],
                rewritten: Vec::new(),
                replica: 1,
                cause: UnrepairedReplica::Unwritten(Box::new(
                    CommitError::Storage(trash_refusal()),
                )),
            },
            Vec::new(),
        ));

        let [chain, advice] = &failed.said[..] else {
            panic!(
                "expected the chain and then the advice, got {:?}",
                failed.said
            );
        };
        assert!(
            chain.ends_with("these credentials may write but not delete"),
            "the chain runs down to what Storage said: {chain}",
        );
        assert!(!chain.contains("running again"), "{chain}");
        assert_eq!(
            advice,
            "running again examines the committed Keyring and repairs it afresh"
        );
    }

    // The other refusal that advises under a cause: a folder create whose
    // answer was lost, where to look is said after the step's own failure.
    #[test]
    fn a_lost_folder_create_says_where_to_look_after_the_cause() {
        let failed = failed(
            &Error::LibraryNotCreated {
                name: "holiday-photos".to_owned(),
                step: CreationStep::AppFolder,
                orphan_folder: None,
                cause: Box::new(Error::NoStateDirectory),
            }
            .into(),
        );

        assert!(failed.repaired.is_empty());
        let [_, advice] = &failed.said[..] else {
            panic!(
                "expected the chain and then the advice, got {:?}",
                failed.said
            );
        };
        assert!(
            advice.starts_with("look for a `coffret-` folder"),
            "{advice}"
        );
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
