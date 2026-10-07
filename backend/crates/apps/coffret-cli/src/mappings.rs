//! Listing what this device has mapped.

use clap::Args;
use coffret_device::MappingListing;

use crate::answer::{Answer, Failure, Form, Mappings, Ran};

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
/// the refusal and the need to preserve the original file, so a script reading
/// standard output sees the same two columns either way. Under `--json` the
/// listing is in the answer instead, with the refusal beside it as `refused`.
pub async fn run(args: MappingsArgs, form: Form) -> anyhow::Result<Ran> {
    let listing = coffret_device::mappings(&args.library).await?;
    let mappings = listing.mappings();
    if mappings.is_empty() {
        eprintln!("Nothing is mapped yet.");
    } else if form.is_text() {
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
        eprintln!(
            "Any mappings listed here are only part of this device's records. \
             Keep the Index file and its spools intact; use a compatible build or a \
             migration that preserves device-local records. Recreating mappings and \
             running `coffret sync` cannot recover materialization records or pending work."
        );
    }
    let refused = match &listing {
        MappingListing::Recorded(_) => None,
        MappingListing::FromRefusedFile { refusal, .. } => Some(Failure::of_index(refusal)),
    };
    Ok(Ran::clean(Answer::Mappings(Mappings::new(
        mappings, refused,
    ))))
}
