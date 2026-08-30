use crate::library_args::LibraryArgs;
use crate::passphrase;

pub async fn run(args: LibraryArgs) -> anyhow::Result<()> {
    let passphrase = passphrase::enter(args.passphrase_stdin)?;

    coffret_device::authorize(&args.library, &passphrase, |url| {
        crate::consent::ask("authorize", url)
    })
    .await?;

    eprintln!("The grant is renewed and cached, sealed, in the Library's directory.");
    Ok(())
}
