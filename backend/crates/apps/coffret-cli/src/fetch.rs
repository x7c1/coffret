//! Putting the Library into the mapped folders.

use clap::Args;
use coffret_device::{run_fetch, run_fetch_entry, EntryFetch, EntryPath, FetchOutcome, Findings};

use crate::report::{self, Report};
use coffret_shell::passphrase;

#[derive(Args)]
pub struct FetchArgs {
    /// The Library on this device to fetch from
    #[arg(long)]
    library: String,
    /// The part of the Library to fetch; everything the mappings cover when it
    /// is not given
    #[arg(long)]
    under: Option<String>,
    /// Fetch the one Entry at this path, reading only the part of its Container
    /// that holds it
    #[arg(long, conflicts_with = "under")]
    entry: Option<String>,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    passphrase_stdin: bool,
}

pub async fn run(args: FetchArgs) -> anyhow::Result<Report> {
    // Read before the Passphrase is asked for, and that order is the point: a
    // path with a trailing separator or a `..` in it is the caller's own typo
    // (spec: EP-2), and nobody should type a secret to be told about one.
    let under = args.under.as_deref().map(EntryPath::parse).transpose()?;
    let entry = args.entry.as_deref().map(EntryPath::parse).transpose()?;

    let enter = passphrase::entering(args.passphrase_stdin);

    // One Entry and a folder are different reads rather than the same read
    // narrowed, which is why `--entry` is a different call and not an argument
    // to this one (spec: FM-2, FM-5, PK-16).
    let Some(entry) = entry else {
        let outcome = run_fetch(&args.library, enter, under).await?;
        println!("{}", summary(&outcome));
        return Ok(report::findings(&Findings::from(&outcome)));
    };

    let fetched = run_fetch_entry(&args.library, enter, entry).await?;
    println!("{}", entry_summary(&fetched));
    Ok(report::findings(&Findings::from(&fetched)))
}

/// The one line a person reads to know what the run did.
///
/// The Container count is beside the Entry count because the fetch unit is the
/// whole Container however many of its Entries were wanted (spec: PK-16), so the
/// two differ wherever a Pack held several of them — and the difference is what
/// says the folder was filled out of Packs rather than one file at a time.
fn summary(outcome: &FetchOutcome) -> String {
    format!(
        "fetched {}, containers {}, skipped {}",
        outcome.fetched.len(),
        outcome.containers.len(),
        outcome.skipped,
    )
}

/// The same for a run of one Entry, which has three answers and no counts.
fn entry_summary(fetched: &EntryFetch) -> &'static str {
    match fetched {
        EntryFetch::Placed => "fetched 1, skipped 0",
        // The file is the Entry and there was nothing to fetch (spec: EP-10).
        EntryFetch::AlreadyPresent => "fetched 0, skipped 1",
        EntryFetch::Surfaced(_) => "fetched 0, skipped 0",
    }
}
