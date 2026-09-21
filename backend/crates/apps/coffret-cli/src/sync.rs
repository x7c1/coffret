//! Carrying the mapped folders into the Library.

use coffret_device::{run_sync, Findings, SyncOutcome};

use crate::library_args::LibraryArgs;
use crate::progress::{Reporting, Units};
use crate::report::{self, Report, Unmapped};
use coffret_shell::passphrase;

pub async fn run(args: LibraryArgs) -> anyhow::Result<Report> {
    // A sync of a folder of any size is minutes of walking, encoding and
    // uploading inside one call, and this is what says so while it happens.
    let watching = Reporting::to_stderr(Units::Syncing);
    let outcome = run_sync(
        &args.library,
        passphrase::entering(args.passphrase_stdin),
        &watching,
    )
    .await?;
    // Before the summary, so that what the run answered is not written over
    // the line the run was reporting on.
    watching.finish();

    for line in summary(&outcome) {
        println!("{line}");
    }
    for repaired in report::repaired(outcome.commit.as_ref()) {
        println!("{repaired}");
    }
    Ok(report::findings(&Findings::from(&outcome)))
}

/// What a person reads to know what the run did: the counts, and the one state
/// the counts cannot say.
fn summary(outcome: &SyncOutcome) -> Vec<String> {
    let mut lines = vec![counts(outcome)];
    lines.extend(report::nothing_mapped(
        outcome.mappings,
        Unmapped::NothingToCarryIn,
    ));
    lines
}

/// The one line a person reads to know what the run did.
fn counts(outcome: &SyncOutcome) -> String {
    let committed = report::committed(outcome.commit.as_ref());
    format!(
        "added {}, replaced {}, unchanged {}, {committed}",
        outcome.added.len(),
        outcome.replaced.len(),
        outcome.unchanged,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that carried nothing, on a device holding `mappings` mappings.
    fn empty_run(mappings: usize) -> SyncOutcome {
        SyncOutcome {
            added: Vec::new(),
            replaced: Vec::new(),
            unchanged: 0,
            mappings,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            reconciled: Vec::new(),
            commit: None,
        }
    }

    // The two runs this exists for. A second sync over folders that have not
    // changed, and a device that has mapped nothing at all, produce identical
    // counts — and mean opposite things. The second says so; the first is the
    // ordinary quiet success and reads like one.
    #[test]
    fn a_device_that_maps_nothing_says_something_an_unchanged_folder_does_not() {
        let unmapped = summary(&empty_run(0));
        let unchanged = summary(&empty_run(2));

        assert_eq!(
            unchanged,
            ["added 0, replaced 0, unchanged 0, committed nothing"],
            "a run that found nothing to do is the counts and nothing else",
        );
        assert_eq!(
            unmapped.first().map(String::as_str),
            Some("added 0, replaced 0, unchanged 0, committed nothing"),
            "the counts are still the first line, for whatever reads them",
        );
        assert_eq!(
            unmapped.len(),
            2,
            "and a second line says what the state is"
        );
        let said = &unmapped[1];
        assert!(
            said.contains("maps no folder"),
            "it must say what the state is: {said:?}",
        );
        assert!(
            said.contains("`coffret map`"),
            "and what leaves it: {said:?}",
        );
    }
}
