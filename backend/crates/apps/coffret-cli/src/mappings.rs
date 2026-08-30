//! Listing what this device has mapped.

use clap::Args;

#[derive(Args)]
pub struct MappingsArgs {
    /// The Library on this device to list the mappings of
    #[arg(long)]
    library: String,
}

/// Lists what this device has mapped, the Library root first.
pub async fn run(args: MappingsArgs) -> anyhow::Result<()> {
    let mappings = coffret_device::mappings(&args.library).await?;
    if mappings.is_empty() {
        eprintln!("Nothing is mapped yet.");
        return Ok(());
    }

    for mapping in mappings {
        // The root mapping stands for everything the top-level ones do not, so
        // it is spelled as the Library root rather than as an empty prefix.
        let prefix = match &mapping.prefix {
            Some(prefix) => prefix.as_str(),
            None => "/",
        };
        println!("{prefix}\t{}", mapping.local_root.display());
    }
    Ok(())
}
