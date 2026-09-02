//! Packing the eligible local files in a folder directly into Packs.
//!
//! Eligible is what the pack policy says it is: a file new to the Library, or
//! one whose current Entry a one-file Container holds. An Entry already in a
//! Pack is never among them, because a freeze neither reads existing Packs as
//! input nor rewrites them (spec: PK-1, PK-2).

use clap::Args;
use coffret_device::{run_freeze, EntryPath, Findings, FreezeOutcome, DEFAULT_PACK_TARGET};

use crate::report::{self, Report};
use coffret_shell::passphrase;

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
    ///
    /// The default is the device layer's, shared with the explorer's server so
    /// that a Pack is one size whichever shell asked for it; this flag is what
    /// overrides it for one run.
    #[arg(long, default_value_t = DEFAULT_PACK_TARGET)]
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
