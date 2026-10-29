//! Removing the Journal records the Snapshot of the Library's head covers, and
//! the Keyring generations older than the one it records.

use coffret_device::{run_prune, PruneOutcome, Redacted};

use crate::answer::{Answer, Form, Pruned, Ran};
use crate::library_args::LibraryArgs;
use crate::report::{self, Report};
use coffret_shell::passphrase;

pub async fn run(args: LibraryArgs, form: Form) -> anyhow::Result<Ran> {
    let outcome = run_prune(&args.library(), passphrase::entering(args.passphrase_stdin)).await?;

    report::summary(&said(&outcome), form);
    // A record or a Keyring replica left in place is work for a later run,
    // which is what the findings status says: the run did what it could, and a
    // script that only asked whether the command failed must not read it as
    // the whole job.
    let report = match outcome.unremoved.is_empty() && outcome.unremoved_replicas.is_empty() {
        true => Report::Clean,
        false => Report::Findings,
    };
    Ok(Ran {
        report,
        answer: Answer::Pruned(Pruned::from(&outcome)),
        findings: Vec::new(),
    })
}

/// The summary line, and one line under it per record or Keyring replica left
/// in place.
///
/// Generations rather than object names for records, a replica's control
/// object name, and the reason in its redacted form: what a person needs is
/// which object and what Storage said, and nothing in any of them is a name of
/// theirs.
fn said(outcome: &PruneOutcome) -> Vec<String> {
    let snapshot = match outcome.wrote_snapshot {
        true => format!("wrote checkpoint {}", outcome.snapshot),
        false => format!("checkpoint {} was already there", outcome.snapshot),
    };
    let removed = match outcome.removed_range() {
        Some((oldest, newest)) if oldest == newest => {
            format!("removed 1 Journal record (head {oldest})")
        }
        Some((oldest, newest)) => format!(
            "removed {} Journal records (heads {oldest} to {newest})",
            outcome.removed.len()
        ),
        None => "removed no Journal record".to_owned(),
    };
    let replicas = match outcome.removed_replicas.len() {
        0 => "trashed no superseded Keyring replica".to_owned(),
        1 => "trashed 1 superseded Keyring replica".to_owned(),
        count => format!("trashed {count} superseded Keyring replicas"),
    };
    let mut lines = vec![format!(
        "{snapshot}, {removed}, kept head {}, {replicas}",
        outcome.head
    )];
    lines.extend(outcome.unremoved.iter().map(|unremoved| {
        format!(
            "head {} was not removed ({}); the next prune removes it",
            unremoved.generation,
            unremoved.cause.redacted()
        )
    }));
    lines.extend(outcome.unremoved_replicas.iter().map(|unremoved| {
        format!(
            "Keyring replica {} was not trashed ({}); the next prune trashes it",
            unremoved.replica,
            unremoved.cause.redacted()
        )
    }));
    lines
}

#[cfg(test)]
mod tests {
    use coffret_device::{
        CommitError, ControlObjectName, Generation, StorageError, UnremovedRecord, UnremovedReplica,
    };

    use super::*;

    fn generation(number: u64) -> Generation {
        Generation::new(number).expect("a small generation is one")
    }

    fn refusal() -> CommitError {
        CommitError::Storage(StorageError::Rejected {
            status: 503,
            detail: "the provider's own words".to_owned(),
            source: None,
        })
    }

    fn replica(number: u64, position: u16) -> ControlObjectName {
        ControlObjectName::parse(&format!("key-{number}-ab12-r{position}-of-3.cfrt"))
            .expect("a replica's name parses")
    }

    // The summary says the checkpoint, the range removed, the head kept and the
    // Keyring replicas trashed, and a record or a replica left in place gets a
    // line of its own with Storage's answer in its redacted form.
    #[test]
    fn the_summary_says_the_range_and_each_record_left_in_place() {
        let outcome = PruneOutcome {
            head: generation(9),
            snapshot: ControlObjectName::index_snapshot(generation(9)),
            wrote_snapshot: true,
            covered_through: generation(9),
            removed: (0..=7).filter(|n| *n != 4).map(generation).collect(),
            already_gone: Vec::new(),
            kept: Vec::new(),
            unremoved: vec![UnremovedRecord {
                generation: generation(4),
                cause: refusal(),
            }],
            removed_replicas: vec![replica(7, 0), replica(7, 1)],
            unremoved_replicas: vec![UnremovedReplica {
                replica: replica(7, 2),
                cause: refusal(),
            }],
        };

        let lines = said(&outcome);

        assert_eq!(
            lines[0],
            "wrote checkpoint idx-9.cfrt, removed 7 Journal records (heads 0 to 7), kept head 9, \
             trashed 2 superseded Keyring replicas",
        );
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert!(
            lines[1].starts_with("head 4 was not removed ("),
            "{lines:?}"
        );
        assert!(
            lines[1].ends_with("; the next prune removes it"),
            "{lines:?}"
        );
        assert!(!lines[1].contains("the provider's own words"), "{lines:?}");
        assert!(
            lines[2].starts_with("Keyring replica key-7-ab12-r2-of-3.cfrt was not trashed ("),
            "{lines:?}"
        );
        assert!(
            lines[2].ends_with("; the next prune trashes it"),
            "{lines:?}"
        );
        assert!(!lines[2].contains("the provider's own words"), "{lines:?}");
    }

    // One trashed replica is said in the singular, and none is said too.
    #[test]
    fn the_summary_counts_the_trashed_replicas() {
        let mut outcome = PruneOutcome {
            head: generation(2),
            snapshot: ControlObjectName::index_snapshot(generation(2)),
            wrote_snapshot: false,
            covered_through: generation(2),
            removed: Vec::new(),
            already_gone: Vec::new(),
            kept: Vec::new(),
            unremoved: Vec::new(),
            removed_replicas: Vec::new(),
            unremoved_replicas: Vec::new(),
        };
        assert_eq!(
            said(&outcome),
            [
                "checkpoint idx-2.cfrt was already there, removed no Journal record, kept head 2, \
              trashed no superseded Keyring replica"
            ],
        );

        outcome.removed_replicas.push(replica(1, 0));
        assert!(
            said(&outcome)[0].ends_with(", trashed 1 superseded Keyring replica"),
            "{:?}",
            said(&outcome),
        );
    }
}
