//! Packing the eligible local files in a folder directly into Packs.
//!
//! Eligible is what the pack policy says it is: a file new to the Library, or
//! one whose current Entry a one-file Container holds. An Entry already in a
//! Pack is never among them, because a freeze neither reads existing Packs as
//! input nor rewrites them (spec: PK-1, PK-2).

use clap::Args;
use coffret_device::{
    run_freeze, EntryPath, Findings, FreezeOutcome, DEFAULT_PACK_TARGET, MINIMUM_PACK_TARGET,
};

use crate::progress::{Reporting, Units};
use crate::report::{self, Report, Unmapped};
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
    /// is one gibibyte (1073741824) and the smallest accepted is one mebibyte
    /// (1048576)
    ///
    /// `--target 4` is four bytes and not four gigabytes, and a target that
    /// small is refused: a run under one would cut one Entry per Pack and look
    /// exactly like a run that worked. Four gibibytes is 4294967296.
    ///
    /// The default is the device layer's, shared with the explorer's server so
    /// that a Pack is one size whichever shell asked for it; this flag is what
    /// overrides it for one run.
    #[arg(long, default_value_t = DEFAULT_PACK_TARGET, value_parser = target_in_bytes)]
    target: u64,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does
    #[arg(long)]
    passphrase_stdin: bool,
}

/// The default a person who gives no `--target` gets has to be one this would
/// take from them, or the flag's absence would be refused where its presence is
/// not. Checked here rather than tested, because both are constants.
const _: () = assert!(DEFAULT_PACK_TARGET >= MINIMUM_PACK_TARGET);

/// A Pack target that means something, or a refusal that says what the unit is.
///
/// A parser of its own rather than `clap`'s ranged one, which would refuse the
/// same numbers and say so in terms of a range rather than of the unit — which
/// is the whole of the mistake being caught here. Somebody who typed `4`
/// thinking in gigabytes has to read the word "bytes" to understand what
/// happened, so the refusal says it, and says what the smallest Pack worth
/// cutting is.
fn target_in_bytes(typed: &str) -> Result<u64, String> {
    let bytes: u64 = typed
        .parse()
        .map_err(|_| format!("{typed:?} is not a number of bytes"))?;

    if bytes < MINIMUM_PACK_TARGET {
        return Err(format!(
            "{bytes} bytes is smaller than a Pack can usefully be. --target is a size in \
             bytes before padding, and the smallest accepted is {MINIMUM_PACK_TARGET} — one \
             mebibyte, which is the plaintext a Container's first chunk holds. For a target \
             of four gibibytes, give {}",
            4 * 1024 * 1024 * 1024u64,
        ));
    }
    Ok(bytes)
}

pub async fn run(args: FreezeArgs) -> anyhow::Result<Report> {
    // Read before the Passphrase is asked for, and that order is the point: a
    // path with a trailing separator or a `..` in it is the caller's own typo
    // (spec: EP-2), and nobody should type a secret to be told about one.
    let under = args.under.as_deref().map(EntryPath::parse).transpose()?;

    // A freeze cuts gibibyte-scale Packs and sends them one after another, so
    // this is what says which one it is on while it happens.
    let watching = Reporting::to_stderr(Units::Freezing);
    let outcome = run_freeze(
        &args.library,
        passphrase::entering(args.passphrase_stdin),
        under,
        args.target,
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
fn summary(outcome: &FreezeOutcome) -> Vec<String> {
    let mut lines = vec![counts(outcome)];
    lines.extend(report::nothing_mapped(
        outcome.mappings,
        Unmapped::NothingToCarryIn,
    ));
    lines
}

/// The one line a person reads to know what the run did.
///
/// The already-packed count is on it because it is what tells the ordinary
/// second run over a folder — nothing to do, everything already in Packs
/// (spec: PK-2) — apart from a run that packed nothing for a reason.
fn counts(outcome: &FreezeOutcome) -> String {
    let committed = report::committed(outcome.commit.as_ref());
    format!(
        "packs {} holding {} entries, absorbed {}, packed already {}, {committed}",
        outcome.packs.len(),
        outcome.frozen_entries(),
        outcome.absorbed.len(),
        outcome.packed_already,
    )
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    /// A run that packed nothing, on a device holding `mappings` mappings.
    fn empty_run(mappings: usize) -> FreezeOutcome {
        FreezeOutcome {
            packs: Vec::new(),
            absorbed: Vec::new(),
            packed_already: 0,
            mappings,
            surfaced: Vec::new(),
            unavailable: Vec::new(),
            commit: None,
        }
    }

    // The two runs this exists for. A second freeze over a folder already in
    // Packs (spec: PK-2), and a device that has mapped nothing at all, produce
    // identical counts — and mean opposite things. The second says so; the
    // first is the ordinary quiet success and reads like one.
    #[test]
    fn a_device_that_maps_nothing_says_something_an_already_packed_folder_does_not() {
        let unmapped = summary(&empty_run(0));
        let packed = summary(&empty_run(2));

        assert_eq!(
            packed,
            ["packs 0 holding 0 entries, absorbed 0, packed already 0, committed nothing"],
            "a run with nothing left to pack is the counts and nothing else",
        );
        assert_eq!(
            unmapped.len(),
            2,
            "and a device that maps nothing gets a second line: {unmapped:?}",
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

    /// The flag on its own, so that what clap does with it is what is asserted.
    #[derive(Parser)]
    struct OnlyTheFlag {
        #[command(flatten)]
        args: FreezeArgs,
    }

    /// What `coffret freeze --library main --target <typed>` comes to.
    fn parse(typed: &str) -> Result<u64, String> {
        OnlyTheFlag::try_parse_from(["freeze", "--library", "main", "--target", typed])
            .map(|parsed| parsed.args.target)
            .map_err(|refusal| refusal.to_string())
    }

    // The mistake the floor exists for: a person thinking in gigabytes types a
    // single digit, and without this the run succeeds, cuts one Entry per Pack
    // (spec: PK-3, PK-4), and looks exactly like a run that worked.
    #[test]
    fn a_target_too_small_to_mean_anything_is_refused_in_the_unit_it_is_in() {
        let refusal = parse("4").expect_err("four bytes is not a Pack target");
        assert!(
            refusal.contains("bytes"),
            "the refusal must name the unit, since the unit is the misreading: {refusal}",
        );
        assert!(
            refusal.contains(&MINIMUM_PACK_TARGET.to_string()),
            "and say what the smallest accepted is: {refusal}",
        );
    }

    // The floor itself is accepted: it is the smallest Pack the format makes
    // sense of rather than a round number somebody preferred.
    #[test]
    fn the_smallest_pack_the_format_makes_sense_of_is_taken() {
        assert_eq!(
            parse(&MINIMUM_PACK_TARGET.to_string()),
            Ok(MINIMUM_PACK_TARGET)
        );
    }

    #[test]
    fn an_ordinary_target_is_taken_as_it_was_typed() {
        let four_gibibytes = 4 * 1024 * 1024 * 1024u64;
        assert_eq!(parse(&four_gibibytes.to_string()), Ok(four_gibibytes));
    }

    #[test]
    fn something_that_is_not_a_number_is_refused_as_one() {
        let refusal = parse("4GiB").expect_err("a size with a unit on it is not a number");
        assert!(
            refusal.contains("not a number of bytes"),
            "the refusal must say what was expected: {refusal}",
        );
    }
}
