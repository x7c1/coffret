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

    eprintln!("{RENEWED}");
    Ok(Report::Clean)
}

/// What a person reads once the grant is renewed.
///
/// What they came for, in the words the concept documentation gives them: this
/// device reaches Storage as the account again, and every Library referencing
/// the account is served by the renewed grant from its next run. How the grant
/// is kept on the device is not among them — nothing a person does next
/// depends on it.
const RENEWED: &str = "This device can reach Storage as the account again: every Library that \
                       references the account uses its renewed grant from its next run.";

#[cfg(test)]
mod tests {
    use super::*;

    // The line is pinned because it is the whole of what a person is told, and
    // the wording that regresses is the one that drifts back into how the grant
    // is stored rather than what it lets the device do.
    #[test]
    fn a_renewed_grant_says_what_the_device_can_do_again() {
        assert_eq!(
            RENEWED,
            "This device can reach Storage as the account again: every Library that references \
             the account uses its renewed grant from its next run.",
        );
        for mechanism in ["cached", "sealed"] {
            assert!(
                !RENEWED.contains(mechanism),
                "how the grant is kept is not what a person reads: {RENEWED:?}",
            );
        }
    }
}
