//! Recording that a folder on this device holds part of the Library.

use std::path::PathBuf;

use clap::Args;

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
pub async fn run(args: MapArgs) -> anyhow::Result<()> {
    coffret_device::set_mapping(&args.library, args.prefix.as_deref(), &args.local_root).await?;

    match &args.prefix {
        Some(prefix) => eprintln!("{prefix} is at {}.", args.local_root.display()),
        None => eprintln!("The Library root is at {}.", args.local_root.display()),
    }
    Ok(())
}
