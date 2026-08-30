//! Carrying the mapped folders into the Library.

use coffret_device::{run_sync, Findings, SyncOutcome};

use crate::library_args::LibraryArgs;
use crate::passphrase;
use crate::report::{self, Report};

pub async fn run(args: LibraryArgs) -> anyhow::Result<Report> {
    let outcome = run_sync(&args.library, passphrase::entering(args.passphrase_stdin)).await?;

    println!("{}", summary(&outcome));
    Ok(report::findings(&Findings::from(&outcome)))
}

/// The one line a person reads to know what the run did.
fn summary(outcome: &SyncOutcome) -> String {
    let committed = report::committed(outcome.commit.as_ref());
    format!(
        "added {}, replaced {}, unchanged {}, {committed}",
        outcome.added.len(),
        outcome.replaced.len(),
        outcome.unchanged,
    )
}
