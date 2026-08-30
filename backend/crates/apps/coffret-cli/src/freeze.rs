//! Packing the eligible local files in a folder directly into Packs.
//!
//! Eligible is what the pack policy says it is: a file new to the Library, or
//! one whose current Entry a one-file Container holds. An Entry already in a
//! Pack is never among them, because a freeze neither reads existing Packs as
//! input nor rewrites them (spec: PK-1, PK-2).

use clap::Args;
use coffret_device::{run_freeze, EntryPath, Findings, FreezeOutcome};

use crate::passphrase;
use crate::report::{self, Report};

/// How large a Pack comes out by default, in bytes before padding.
///
/// One gibibyte, and a flag rather than a constant in the byte forms: a Library
/// can be repacked under a different answer, so nothing may come to depend on
/// this one (spec: PK-5, PK-6).
const DEFAULT_TARGET: u64 = 1024 * 1024 * 1024;

#[derive(Args)]
pub struct FreezeArgs {
    /// The Library on this device to freeze
    #[arg(long)]
    library: String,
    /// The top-level part of the Library to freeze; everything the mappings
    /// cover when it is not given
    #[arg(long)]
    under: Option<String>,
    /// How large a Pack should come out, in bytes before padding; the default
    /// is one gibibyte
    #[arg(long, default_value_t = DEFAULT_TARGET)]
    target: u64,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    passphrase_stdin: bool,
}

pub async fn run(args: FreezeArgs) -> anyhow::Result<Report> {
    let outcome = run_freeze(
        &args.library,
        passphrase::entering(args.passphrase_stdin),
        args.under.as_deref().map(EntryPath::nfc),
        args.target,
    )
    .await?;

    println!("{}", summary(&outcome));
    Ok(report::findings(&Findings::from(&outcome)))
}

/// The one line a person reads to know what the run did.
///
/// The already-packed count is on it because it is what tells the ordinary
/// second run over a folder — nothing to do, everything already in Packs
/// (spec: PK-2) — apart from a run that packed nothing for a reason.
fn summary(outcome: &FreezeOutcome) -> String {
    let committed = report::committed(outcome.commit.as_ref());
    format!(
        "packs {} holding {} entries, absorbed {}, packed already {}, {committed}",
        outcome.packs.len(),
        outcome.frozen_entries(),
        outcome.absorbed.len(),
        outcome.packed_already,
    )
}
