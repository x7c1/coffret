//! Giving a file or a folder of the Library a new Entry Path.

use clap::Args;
use coffret_device::{run_rename, EntryPath, Findings, RenameOutcome};

use crate::answer::{Answer, Form, Ran, Renamed};
use crate::progress::{Reporting, Units};
use crate::report;
use coffret_shell::passphrase;

#[derive(Args)]
pub struct RenameArgs {
    /// The Library on this device to rename in
    #[arg(long)]
    library: String,
    /// The Entry Path of the file or folder to rename
    ///
    /// Named rather than positional: a word written after
    /// `--passphrase-stdin` is refused, before anything else reads the
    /// arguments, as a Passphrase typed where it would be seen.
    #[arg(long)]
    from: String,
    /// The Entry Path to give it; a folder's files keep the rest of their
    /// paths under it
    #[arg(long)]
    to: String,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    passphrase_stdin: bool,
}

pub async fn run(args: RenameArgs, form: Form) -> anyhow::Result<Ran> {
    // Read before the Passphrase is asked for, for the reason `fetch` reads its
    // paths first: a path with a trailing separator or a `..` in it is the
    // caller's own typo (spec: EP-1, EP-2), and nobody should type a secret to
    // be told about one.
    let from = EntryPath::parse(args.from)?;
    let to = EntryPath::parse(args.to)?;

    // Nothing is uploaded, but the catch-up and the commit are round trips to
    // Storage, seconds apiece on a provider such as Drive.
    let watching = Reporting::to_stderr(Units::Renaming);
    let outcome = run_rename(
        &args.library,
        passphrase::entering(args.passphrase_stdin),
        from,
        to,
        &watching,
    )
    .await?;
    // Before the summary, so that what the run answered is not written over
    // the line the run was reporting on.
    watching.finish();

    report::summary(&[summary(&outcome)], form);
    let findings = Findings::from(&outcome);
    let report = report::findings(&findings, form);
    Ok(Ran::found(
        report,
        Answer::Renamed(Renamed::from(&outcome)),
        &findings,
    ))
}

/// The one line a person reads to know what the run did.
///
/// A count rather than the paths: the person typed them, and a folder of a
/// thousand files would be a thousand lines. What became of this device's own
/// files is said by the findings under it, the way a sync says it.
fn summary(outcome: &RenameOutcome) -> String {
    let committed = report::committed(Some(&outcome.commit));
    format!("renamed {}, {committed}", outcome.renamed.len())
}
