//! Recording that a folder on this device holds part of the Library.

use std::path::PathBuf;

use clap::Args;

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
pub async fn run(args: MapArgs) -> anyhow::Result<Report> {
    let replaced =
        coffret_device::set_mapping(&args.library, args.prefix.as_deref(), &args.local_root)
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
    match replaced {
        Some(mapping) => eprintln!(
            "{what} was at {}; it is now at {now}.",
            mapping.local_root.display()
        ),
        None => eprintln!("{what} is at {now}."),
    }
    Ok(Report::Clean)
}
