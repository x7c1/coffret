//! Renewing the grant of an account this device holds.

use clap::{ArgGroup, Args};
use coffret_device::AuthorizeRequest;

use crate::Report;
use coffret_shell::passphrase;

/// Which grant to renew: an account's, named by itself or by a Library that
/// references it.
#[derive(Args)]
#[command(group(ArgGroup::new("grant").required(true).multiple(true).args(["library", "account"])))]
pub struct AuthorizeArgs {
    /// A Library on this device: the grant of the account it references is
    /// renewed, for every Library that references that account
    #[arg(long)]
    library: Option<String>,
    /// An account on this device, by the name it was given: its grant is
    /// renewed for every Library that references it. With --library, the
    /// account to bring a Library that references none onto: 1 to 64 ASCII
    /// letters, digits, '-' or '_'. Optional while the device holds one account
    /// and required once it holds more; a name the device does not hold yet is
    /// a new account, consented to here. A Library that already references an
    /// account takes only that one's name, since a name cannot be changed yet
    #[arg(long)]
    account: Option<String>,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it, which is what a script does. For --account alone, the
    /// Passphrase of any Library that references the account
    #[arg(long)]
    passphrase_stdin: bool,
}

pub async fn run(args: AuthorizeArgs) -> anyhow::Result<Report> {
    let request = match (args.library, args.account) {
        (Some(name), account) => AuthorizeRequest::Library { name, account },
        (None, Some(name)) => AuthorizeRequest::Account { name },
        (None, None) => unreachable!("clap requires --library or --account"),
    };
    coffret_device::authorize(
        request,
        passphrase::entering(args.passphrase_stdin),
        |url| crate::consent::ask("authorize", url),
    )
    .await?;

    eprintln!(
        "The grant is renewed and cached, sealed, for the account on this device; every Library \
         that references the account uses it from its next run."
    );
    Ok(Report::Clean)
}
