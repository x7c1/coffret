//! Listing what this device has mapped.

use clap::Args;
use coffret_device::MappingListing;

use crate::Report;

#[derive(Args)]
pub struct MappingsArgs {
    /// The Library on this device to list the mappings of
    #[arg(long)]
    library: String,
}

/// Lists what this device has mapped, the Library root first.
///
/// A Library whose Index this build cannot open is not a dead end for this:
/// the mappings still come out on standard output, because the file gives
/// them up whatever else about its layout is refused. Standard error carries
/// the refusal and the recovery in that case — as commands, which is this
/// layer's to name rather than the device crate's — so a script reading
/// standard output sees the same two columns either way.
pub async fn run(args: MappingsArgs) -> anyhow::Result<Report> {
    let listing = coffret_device::mappings(&args.library).await?;
    let mappings = listing.mappings();
    if mappings.is_empty() {
        eprintln!("Nothing is mapped yet.");
    } else {
        for mapping in mappings {
            // The root mapping stands for everything the top-level ones do
            // not, so it is spelled as the Library root rather than as an
            // empty prefix.
            let prefix = match &mapping.prefix {
                Some(prefix) => prefix.as_str(),
                None => "/",
            };
            println!("{prefix}\t{}", mapping.local_root.display());
        }
    }

    if let MappingListing::FromRefusedFile { refusal, .. } = &listing {
        eprintln!();
        eprintln!("{refusal}");
        if mappings.is_empty() {
            // Nothing above to retype: the file held no mappings even before
            // this build refused it, so the recovery is the Index file alone.
            eprintln!(
                "This Library's Index cannot be opened by this build, and it had no mappings \
                 recorded to read back. To recover: delete the Index file and finish with \
                 `coffret sync`."
            );
        } else {
            eprintln!(
                "This Library's Index cannot be opened by this build; the mappings above were \
                 read directly from the file instead of through its catalog. To recover: \
                 delete the Index file, then `coffret map` each one above back in — with \
                 `--prefix <prefix>` for every line but `/`, which needs none — and finish \
                 with `coffret sync`."
            );
        }
    }
    Ok(Report::Clean)
}
