//! Removing the Journal records the Snapshot of the Library's head covers.

use coffret_device::{run_prune, PruneOutcome, Redacted};

use crate::answer::{Answer, Form, Pruned, Ran};
use crate::library_args::LibraryArgs;
use crate::report::{self, Report};
use coffret_shell::passphrase;

pub async fn run(args: LibraryArgs, form: Form) -> anyhow::Result<Ran> {
    let outcome = run_prune(&args.library(), passphrase::entering(args.passphrase_stdin)).await?;

    report::summary(&said(&outcome), form);
    // A record left in place is work for a later run, which is what the
    // findings status says: the run did what it could, and a script that only
    // asked whether the command failed must not read it as the whole job.
    let report = match outcome.unremoved.is_empty() {
        true => Report::Clean,
        false => Report::Findings,
    };
    Ok(Ran {
        report,
        answer: Answer::Pruned(Pruned::from(&outcome)),
        findings: Vec::new(),
    })
}

/// The summary line, and one line under it per record left in place.
///
/// Generations rather than object names, and the reason in its redacted form:
/// what a person needs is which record and what Storage said, and nothing in
/// either is a name of theirs.
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
    let mut lines = vec![format!("{snapshot}, {removed}, kept head {}", outcome.head)];
    lines.extend(outcome.unremoved.iter().map(|unremoved| {
        format!(
            "head {} was not removed ({}); the next prune removes it",
            unremoved.generation,
            unremoved.cause.redacted()
        )
    }));
    lines
}

#[cfg(test)]
mod tests {
    use coffret_device::{
        CommitError, ControlObjectName, Generation, StorageError, UnremovedRecord,
    };

    use super::*;

    fn generation(number: u64) -> Generation {
        Generation::new(number).expect("a small generation is one")
    }

    // The summary says the checkpoint, the range removed and the head kept, and
    // a record left in place gets a line of its own with Storage's answer in
    // its redacted form.
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
                cause: CommitError::Storage(StorageError::Rejected {
                    status: 503,
                    detail: "the provider's own words".to_owned(),
                    source: None,
                }),
            }],
        };

        let lines = said(&outcome);

        assert_eq!(
            lines[0],
            "wrote checkpoint idx-9.cfrt, removed 7 Journal records (heads 0 to 7), kept head 9",
        );
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(
            lines[1].starts_with("head 4 was not removed ("),
            "{lines:?}"
        );
        assert!(
            lines[1].ends_with("; the next prune removes it"),
            "{lines:?}"
        );
        assert!(!lines[1].contains("the provider's own words"), "{lines:?}");
    }
}
