use crate::library_args::LibraryArgs;
use crate::passphrase;
use crate::Report;

pub async fn run(args: LibraryArgs) -> anyhow::Result<Report> {
    coffret_device::authorize(
        &args.library,
        passphrase::entering(args.passphrase_stdin),
        |url| crate::consent::ask("authorize", url),
    )
    .await?;

    eprintln!("The grant is renewed and cached, sealed, in the Library's directory.");
    Ok(Report::Clean)
}
