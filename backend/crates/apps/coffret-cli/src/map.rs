//! Recording that a folder on this device holds part of the Library.

use std::path::PathBuf;

use clap::Args;
use coffret_device::{MarkerRecord, MarkerRequest};

use crate::Report;

#[derive(Args)]
pub struct MapArgs {
    /// The Library on this device to record the mapping in
    #[arg(long)]
    library: String,
    /// The top-level part of the Library this folder holds; the Library root
    /// when it is not given
    #[arg(long)]
    prefix: Option<String>,
    /// Give this folder a new identity of its own instead of keeping the one it
    /// already carries; for two folders that ended up sharing an identity
    /// because one was copied from the other
    #[arg(long)]
    reset_marker: bool,
    /// The folder on this device
    local_root: PathBuf,
}

/// Records a mapping. No Passphrase: a mapping is device state in a plaintext
/// catalog and says nothing the Library keeps secret.
///
/// A prefix that was already mapped is moved rather than added to, and what it
/// stood for is said back: everything under the old root leaves the Library's
/// reach on this device the moment the mapping moves, and a person who typed the
/// wrong prefix would otherwise have no way of noticing.
///
/// What became of the folder's own identity is said back for a related reason.
/// This command is the only one that writes a marker into a folder, and whether
/// it wrote one, found one and kept it, or replaced one is what tells a person
/// whether the folder in front of them is the folder they registered before
/// (spec: EP-13) — which a line saying only where the mapping now points would
/// leave them guessing at.
pub async fn run(args: MapArgs) -> anyhow::Result<Report> {
    let marker = match args.reset_marker {
        true => MarkerRequest::IssueANewIdentity,
        false => MarkerRequest::AdoptWhatIsThere,
    };
    let recorded = coffret_device::set_mapping(
        &args.library,
        args.prefix.as_deref(),
        &args.local_root,
        marker,
    )
    .await?;

    let what = match &args.prefix {
        Some(prefix) => prefix.clone(),
        None => "The Library root".to_owned(),
    };
    // The root as it was recorded rather than as it was typed: a mapping outlives
    // the working directory the command ran in, so the device layer resolves the
    // folder before storing it — and a sentence whose two halves stood in
    // different forms would read as a move between two folders that are one.
    let now = args
        .local_root
        .canonicalize()
        .unwrap_or_else(|_| args.local_root.clone());
    let now = now.display();
    match recorded.replaced {
        Some(mapping) => eprintln!(
            "{what} was at {}; it is now at {now}.",
            mapping.local_root.display()
        ),
        None => eprintln!("{what} is at {now}."),
    }
    // The folder's identity, in the same voice and on the same stream. The
    // folder rather than "it": the sentence above is about the prefix and ends
    // by saying where it now stands, so a pronoun here would take the prefix for
    // what carries an identity. The adopted case is the one worth reading twice:
    // it says this folder had already been registered — by another mapping, or
    // by another device — and that recording it again took nothing away from
    // either, which is also why the flag that would take it away is offered with
    // the one situation that calls for it rather than on its own.
    match recorded.marker {
        MarkerRecord::Written => {
            eprintln!("That folder carried no identity, so one was written into it.")
        }
        MarkerRecord::Adopted => eprintln!(
            "That folder already carries an identity, which was kept as it was: whatever \
             registered it before — another mapping, or another device — still matches it. Pass \
             --reset-marker only where it is a copy that has to be told apart from the folder it \
             was copied from."
        ),
        MarkerRecord::Reset => eprintln!(
            "That folder was given a new identity, so anything registered against the old one no \
             longer matches it."
        ),
    }
    Ok(Report::Clean)
}
