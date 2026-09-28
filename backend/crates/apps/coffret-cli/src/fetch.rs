//! Putting the Library into the mapped folders.

use clap::Args;
use coffret_device::{
    run_fetch, run_fetch_entry, EntryFetch, EntryPath, Error as DeviceError, FetchError,
    FetchOutcome, Findings,
};

use crate::progress::{Reporting, Units};
use crate::report::{self, Report, Unmapped};
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
        // A fetch of a folder pulls whole Containers back one after another and
        // says nothing for as long as that takes; this is what says it is
        // moving.
        let watching = Reporting::to_stderr(Units::Fetching);
        let outcome = run_fetch(&args.library, enter, under, &watching).await?;
        // Before the summary, so that what the run answered is not written over
        // the line the run was reporting on.
        watching.finish();

        for line in summary(&outcome) {
            println!("{line}");
        }
        return Ok(report::findings(&Findings::from(&outcome)));
    };

    // A range read out of a Pack can still be megabytes, after the same
    // catch-up a folder fetch starts with, so one Entry says it is moving too.
    let watching = Reporting::to_stderr(Units::Fetching);
    let fetched = run_fetch_entry(&args.library, enter, entry, &watching)
        .await
        .map_err(next_step)?;
    watching.finish();

    println!("{}", entry_summary(&fetched.fetch));
    Ok(report::findings(&Findings::from(&fetched)))
}

/// What a person reads to know what the run did: the counts, and the one state
/// the counts cannot say.
fn summary(outcome: &FetchOutcome) -> Vec<String> {
    let mut lines = vec![counts(outcome)];
    lines.extend(report::nothing_mapped(
        outcome.mappings,
        Unmapped::NowhereForTheLibraryToGo,
    ));
    lines
}

/// The one line a person reads to know what the run did.
///
/// The Container count is beside the Entry count because the fetch unit is the
/// whole Container however many of its Entries were wanted (spec: PK-16), so the
/// two differ wherever a Pack held several of them — and the difference is what
/// says the folder was filled out of Packs rather than one file at a time.
fn counts(outcome: &FetchOutcome) -> String {
    format!(
        "fetched {}, containers {}, skipped {}",
        outcome.fetched.len(),
        outcome.containers.len(),
        outcome.skipped,
    )
}

/// The refusal a one-Entry fetch failed with, and the way past it where the
/// shell can name one.
///
/// A path no mapping of this device reaches is the state a folder fetch meets
/// as a device mapping nothing, and the way out of both is the same command
/// (see [`report::nothing_mapped`]). The flow's sentence says what the state
/// is and stays as it is; which command leaves it is this shell's vocabulary,
/// so it is said here, on a line after that sentence rather than inside it.
fn next_step(error: DeviceError) -> anyhow::Error {
    let unmapped = matches!(
        &error,
        DeviceError::Fetch { cause } if matches!(cause.as_ref(), FetchError::UnmappedEntryPath { .. })
    );
    let error = anyhow::Error::new(error);
    match unmapped {
        true => anyhow::anyhow!("{error:#}\n{UNMAPPED_ENTRY}"),
        false => error,
    }
}

/// What a person does about an Entry no mapping of this device reaches.
const UNMAPPED_ENTRY: &str = "to put it on this device, record a mapping that reaches it with \
                              `coffret map` and run this again";

/// The same for a run of one Entry, which has three answers and no counts.
fn entry_summary(fetched: &EntryFetch) -> &'static str {
    match fetched {
        EntryFetch::Placed => "fetched 1, skipped 0",
        // The file is the Entry and there was nothing to fetch (spec: EP-10).
        EntryFetch::AlreadyPresent => "fetched 0, skipped 1",
        EntryFetch::Surfaced(_) => "fetched 0, skipped 0",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that placed nothing, out of `mappings` mappings.
    fn empty_run(mappings: usize) -> FetchOutcome {
        FetchOutcome {
            fetched: Vec::new(),
            containers: Vec::new(),
            skipped: 0,
            mappings,
            surfaced: Vec::new(),
            refused: Vec::new(),
            locked: Vec::new(),
            degraded: None,
        }
    }

    // The two runs this exists for. `--under` naming a prefix the Library holds
    // nothing under, and a device that has mapped nothing at all, produce
    // identical counts — and mean opposite things. The second says so; the
    // first is an ordinary empty answer and reads like one.
    #[test]
    fn a_device_that_maps_nothing_says_something_an_empty_prefix_does_not() {
        let unmapped = summary(&empty_run(0));
        let empty_prefix = summary(&empty_run(2));

        assert_eq!(
            empty_prefix,
            ["fetched 0, containers 0, skipped 0"],
            "an empty answer about the Library is the counts and nothing else",
        );
        assert_eq!(
            unmapped.first().map(String::as_str),
            Some("fetched 0, containers 0, skipped 0"),
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
        assert!(said.contains("coffret map"), "and what leaves it: {said:?}",);
    }

    // A one-Entry fetch whose path no mapping reaches ends in the flow's
    // refusal, and the shell says under it which command leaves that state —
    // the way a folder fetch on a device mapping nothing does.
    #[test]
    fn an_unmapped_entry_says_to_record_a_mapping_after_the_refusal() {
        let path = EntryPath::parse("albums/a.jpg").expect("the literal is one");
        let refusal = FetchError::UnmappedEntryPath { path: path.clone() }.to_string();
        let said = format!(
            "{:#}",
            next_step(DeviceError::Fetch {
                cause: Box::new(FetchError::UnmappedEntryPath { path }),
            })
        );

        let lines: Vec<&str> = said.lines().collect();
        assert_eq!(lines.len(), 2, "the refusal, then the next step: {said:?}");
        assert!(
            lines[0].ends_with(&refusal),
            "the flow's sentence is kept as it is: {said:?}",
        );
        assert!(
            lines[1].contains("coffret map"),
            "and the line after it names the command that leaves the state: {said:?}",
        );
    }

    // Every other refusal reads as it always did.
    #[test]
    fn any_other_refusal_is_left_as_it_is() {
        let path = EntryPath::parse("albums/a.jpg").expect("the literal is one");
        let said = format!(
            "{:#}",
            next_step(DeviceError::Fetch {
                cause: Box::new(FetchError::EntryNotCurrent { path }),
            })
        );
        assert!(!said.contains("coffret map"), "{said:?}");
        assert_eq!(said.lines().count(), 1, "{said:?}");
    }

    // A run that placed something says the counts and nothing else, whatever
    // the mappings: the line above is about a device with none.
    #[test]
    fn a_mapped_device_reads_as_it_always_did() {
        let outcome = FetchOutcome {
            fetched: vec![EntryPath::parse("albums/a.jpg").expect("the literal is one")],
            containers: Vec::new(),
            skipped: 0,
            mappings: 1,
            surfaced: Vec::new(),
            refused: Vec::new(),
            locked: Vec::new(),
            degraded: None,
        };
        assert_eq!(summary(&outcome), ["fetched 1, containers 0, skipped 0"]);
    }
}
