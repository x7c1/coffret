//! Rewriting the Packs that hold a folder's files into Packs cut in path order.
//!
//! A freeze groups files by path only within one run, and a rename leaves a
//! file in the Pack it was frozen into, so a folder's files come to be spread
//! across Packs that also hold other folders' files (spec: PK-8). This is the
//! command that puts them back in order, and it runs only when asked
//! (spec: PK-23).

use clap::Args;
use coffret_device::{run_repack, EntryPath, Findings, RepackOutcome};

use crate::answer::{Answer, Form, Ran, Repacked};
use crate::freeze::target_in_bytes;
use crate::library_args::LibraryArgs;
use crate::progress::{Reporting, Units};
use crate::report;
use coffret_device::DEFAULT_PACK_TARGET;
use coffret_shell::passphrase;

/// What `--under` takes to name the Library root.
const ROOT: &str = "/";

#[derive(Args)]
pub struct RepackArgs {
    // The Library on this device, and how its Passphrase is read.
    #[command(flatten)]
    library: LibraryArgs,
    /// The folder of the Library whose Packs to repack, e.g. `books/series`;
    /// `/` names the whole Library
    ///
    /// Required, and the whole Library is named rather than assumed: a repack
    /// of it rewrites every Pack not already in path order, which is asked for
    /// on purpose or not at all.
    #[arg(long)]
    under: String,
    /// How large a Pack should come out, in bytes before padding; the default
    /// is one gibibyte (1073741824) and the smallest accepted is one mebibyte
    /// (1048576)
    ///
    /// The same target `freeze` cuts under: a folder repacked under the
    /// target it was frozen under keeps every Pack that is already the cut.
    #[arg(long, default_value_t = DEFAULT_PACK_TARGET, value_parser = target_in_bytes)]
    target: u64,
}

pub async fn run(args: RepackArgs, form: Form) -> anyhow::Result<Ran> {
    // Read before the Passphrase is asked for, for the reason `freeze` reads
    // its folder first: a path with a trailing separator or a `..` in it is
    // the caller's own typo (spec: EP-2), and nobody should type a secret to be
    // told about one.
    let folder = folder(&args.under)?;

    // Every Pack it rewrites is read whole and written again, so this is what
    // says which one it is on while it happens.
    let watching = Reporting::to_stderr(Units::Repacking);
    let outcome = run_repack(
        &args.library.library(),
        passphrase::entering(args.library.passphrase_stdin),
        folder,
        args.target,
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
        Answer::Repacked(Repacked::from(&outcome)),
        &findings,
    ))
}

/// The folder `--under` names: an Entry Path, or `None` for `/`, the Library
/// root.
fn folder(typed: &str) -> Result<Option<EntryPath>, coffret_device::ModelError> {
    match typed {
        ROOT => Ok(None),
        typed => EntryPath::parse(typed).map(Some),
    }
}

/// The one line a person reads to know what the run did.
///
/// The kept count is on it because it is what tells a folder already in order
/// — the ordinary second run — apart from one that selected nothing at all.
fn summary(outcome: &RepackOutcome) -> String {
    let committed = report::committed(outcome.commit.as_ref());
    format!(
        "selected {} packs, kept {}, removed {}, added {}, uploaded {} bytes, {committed}",
        outcome.selected.len(),
        outcome.kept.len(),
        outcome.removed.len(),
        outcome.added.len(),
        outcome.uploaded(),
    )
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use coffret_device::{ContainerId, RepackedPack};

    use super::*;

    fn id(byte: u8) -> ContainerId {
        ContainerId::from_bytes([byte; ContainerId::BYTE_LEN])
    }

    // The counts, the bytes sent and whether anything was committed — and a
    // folder already in order reads as kept Packs and nothing committed.
    #[test]
    fn the_summary_says_the_counts_and_the_bytes() {
        let rewritten = RepackOutcome {
            selected: vec![id(1), id(2), id(3)],
            kept: vec![id(3)],
            removed: vec![id(1), id(2)],
            added: vec![RepackedPack {
                container_id: id(4),
                entries: 7,
                footprint: 900,
                written: 1_024,
            }],
            commit: None,
            short: None,
        };
        assert_eq!(
            summary(&rewritten),
            "selected 3 packs, kept 1, removed 2, added 1, uploaded 1024 bytes, committed nothing",
        );

        let in_order = RepackOutcome {
            selected: vec![id(3)],
            kept: vec![id(3)],
            removed: Vec::new(),
            added: Vec::new(),
            commit: None,
            short: None,
        };
        assert_eq!(
            summary(&in_order),
            "selected 1 packs, kept 1, removed 0, added 0, uploaded 0 bytes, committed nothing",
        );
    }

    // `/` is the Library root, and anything else is an Entry Path held to the
    // rules every path is (spec: EP-2).
    #[test]
    fn a_slash_names_the_library_root_and_anything_else_a_folder() {
        assert_eq!(folder("/").expect("the root"), None);
        assert_eq!(
            folder("books/series")
                .expect("a folder")
                .map(|path| path.as_str().to_owned()),
            Some("books/series".to_owned()),
        );
        assert!(folder("books/").is_err(), "a trailing separator is refused");
    }

    /// The command's flags on their own.
    #[derive(Parser)]
    struct OnlyTheFlags {
        #[command(flatten)]
        args: RepackArgs,
    }

    // The folder is never assumed: a repack of the whole Library is asked for
    // by naming it.
    #[test]
    fn under_is_required() {
        let refusal = OnlyTheFlags::try_parse_from(["repack", "--library", "main"])
            .err()
            .expect("a repack names its folder");
        assert!(refusal.to_string().contains("--under"), "{refusal}");
    }
}
