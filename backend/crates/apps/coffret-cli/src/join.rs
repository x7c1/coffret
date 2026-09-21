//! Taking up a Library another device created.

use anyhow::bail;
use clap::{ArgGroup, Args};
use coffret_device::{
    join_library, FoundOnStorage, JoinLibraryRequest, JoinedLibrary, JoinedProvider,
};

use crate::drive_client;
use crate::storage_location::storage;
use crate::Report;
use coffret_shell::{passphrase, recovery_code};

/// Exactly one provider, and only the flags that provider has.
///
/// The same shape `init` takes, with one difference that is the whole of what
/// joining means: the flags name the Library's *own* folder rather than
/// somewhere to make one. `--folder-id` is the app folder itself, and
/// `--prefix` is the whole prefix ending in `coffret-<library id>/` — which is
/// what `init` printed and what the provider's own interface shows.
#[derive(Args)]
#[command(group(ArgGroup::new("provider").required(true).args(["drive", "s3"])))]
pub struct JoinArgs {
    /// What this device is to call the Library
    #[arg(long)]
    name: String,

    /// The Library is in Google Drive
    #[arg(long, requires = "folder_id", requires = "client_id")]
    drive: bool,
    /// The Library's own folder on Drive, by the id Drive minted for it — the
    /// one in that folder's address in Drive, and the one `init` printed
    #[arg(long, conflicts_with = "s3")]
    folder_id: Option<String>,
    /// The OAuth desktop client to authorize as, by the id of a desktop client
    /// registered in the account owner's own Cloud project; required, because
    /// coffret has no built-in one and this id decides which application this
    /// device reaches the Library as. The client secret, where that client was
    /// registered with one, is read from COFFRET_DRIVE_CLIENT_SECRET rather
    /// than typed: it is configuration this Library stores and re-reads on
    /// every token refresh, and a flag would leave it in the shell history and
    /// the process table
    ///
    /// One way to put that secret in the environment without the shell
    /// history keeping a copy of it:
    ///
    ///   read -rs COFFRET_DRIVE_CLIENT_SECRET && export COFFRET_DRIVE_CLIENT_SECRET
    ///   coffret join --name NAME --drive --folder-id FOLDER_ID --client-id CLIENT_ID
    ///   unset COFFRET_DRIVE_CLIENT_SECRET
    #[arg(long, conflicts_with = "s3", verbatim_doc_comment)]
    client_id: Option<String>,

    /// The Library is in an S3 bucket; the credentials are whichever the AWS
    /// SDK resolves — the environment, then a profile — and none is asked for
    /// here
    #[arg(long, requires = "bucket", requires = "prefix")]
    s3: bool,
    /// The bucket the Library is in
    #[arg(long, conflicts_with = "drive")]
    bucket: Option<String>,
    /// The Library's own prefix: the base prefix it was created under with
    /// `coffret-<library id>/` after it, ending in `/`. Not the base prefix
    /// itself — that is what `init --prefix` takes, and it holds every Library
    /// kept at that location rather than this one
    #[arg(long, conflicts_with = "drive")]
    prefix: Option<String>,
    /// The S3 endpoint to talk to, where it is not AWS's own
    #[arg(long, conflicts_with = "drive")]
    endpoint: Option<String>,
    /// The region to sign for, where the AWS SDK's own resolution is not to
    /// decide it
    #[arg(long, conflicts_with = "drive")]
    region: Option<String>,
    /// Address the bucket as a path segment rather than as a subdomain
    #[arg(long, conflicts_with = "drive")]
    path_style: bool,

    // Last, and next to each other: `--help` then lists the two secrets in the
    // order standard input has to carry them.
    /// Read the Recovery Code from one line of standard input instead of asking
    /// for it without echo. With --passphrase-stdin, give the Recovery Code on
    /// the first line and this device's Passphrase on the next
    #[arg(long)]
    recovery_code_stdin: bool,
    /// Read the Passphrase from one line of standard input instead of asking
    /// for it twice, which is what a script does
    #[arg(long)]
    passphrase_stdin: bool,
}

pub async fn run(args: JoinArgs) -> anyhow::Result<Report> {
    let provider = provider(&args)?;

    // Chosen twice rather than entered once: the Passphrase is this device's
    // own, not the one the Library was created under (spec: DK-6), and there is
    // nothing here to check a typo against — the stored form this makes is this
    // device's alone (spec: KD-9).
    let joined = join_library(
        JoinLibraryRequest {
            name: args.name,
            provider,
        },
        recovery_code::entering(args.recovery_code_stdin),
        passphrase::choosing(args.passphrase_stdin),
        |url| crate::consent::ask("join", url),
    )
    .await
    .map_err(drive_client::explaining)?;

    report(&joined);
    Ok(Report::Clean)
}

/// What the flags say about where the Library already is.
fn provider(args: &JoinArgs) -> anyhow::Result<JoinedProvider> {
    if args.drive {
        // `--drive` requires both of these, so clap has already refused the
        // shapes this could otherwise be missing.
        let (Some(folder_id), Some(client_id)) = (args.folder_id.clone(), args.client_id.clone())
        else {
            bail!("--drive needs --folder-id and --client-id");
        };
        let client_secret = drive_client::client_secret()?;
        return Ok(JoinedProvider::Drive {
            folder_id,
            client_id,
            client_secret,
        });
    }

    let (Some(bucket), Some(prefix)) = (args.bucket.clone(), args.prefix.clone()) else {
        bail!("--s3 needs --bucket and --prefix");
    };
    Ok(JoinedProvider::S3 {
        bucket,
        prefix,
        endpoint: args.endpoint.clone(),
        region: args.region.clone(),
        path_style: args.path_style,
    })
}

/// Says what this device now holds, and what it does not.
///
/// No Recovery Code: the one that went in is the one that exists, and printing
/// it back would put a second copy of the Master Key on a terminal that did not
/// ask for one. The catalog is empty until the first run, which is worth saying
/// so that nobody reads `mappings` or a first `fetch` as a Library that turned
/// out to be empty.
fn report(joined: &JoinedLibrary) {
    eprintln!("\nThe Library is at {}.", joined.path.display());
    eprintln!("Library ID: {}", joined.settings.library_id);
    eprintln!("On Storage: {}", storage(&joined.settings.provider));
    if let Some(said) = nothing_there_yet(joined.found) {
        eprintln!("\n{said}");
    }
    eprintln!(
        "\nNothing of the Library is on this device yet. Map a folder, then run \
         `coffret fetch`."
    );
}

/// What a person is told when the place they joined holds nothing of a Library.
///
/// Both of the things this can mean are worth hearing, and the sentence has to
/// serve both because nothing here can tell them apart: a Library created a
/// minute ago and not yet synced holds nothing, and so does a prefix with a
/// character wrong in its Library ID. The first person is told why their
/// Library looks empty; the second finds out now rather than after a `fetch`
/// that reports nothing and exits successfully.
///
/// It is not a refusal. The join has happened either way, and refusing the
/// first person's perfectly good Library to catch the second's typo would be
/// the worse trade.
fn nothing_there_yet(found: FoundOnStorage) -> Option<&'static str> {
    match found {
        FoundOnStorage::TheLibrary => None,
        FoundOnStorage::NothingYet => Some(
            "Storage holds nothing of this Library yet. That is what a Library nobody has \
             synced looks like — and also what somewhere that is not this Library's looks \
             like, so check the place above if you expected it to hold something.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A Library whose place holds something says nothing extra: the ordinary
    // join is the one that works, and a line about it would be noise.
    #[test]
    fn a_library_that_is_there_is_not_remarked_on() {
        assert_eq!(nothing_there_yet(FoundOnStorage::TheLibrary), None);
    }

    // The one sentence both of the empty cases get, because from here they are
    // the same answer: a Library created and never synced, and a prefix that is
    // not this Library's at all.
    #[test]
    fn a_place_holding_nothing_is_said_to_hold_nothing_and_why_that_is_two_things() {
        let said = nothing_there_yet(FoundOnStorage::NothingYet)
            .expect("an empty place is worth a sentence");
        assert!(
            said.contains("nothing of this Library yet"),
            "it must say what was found: {said}",
        );
        assert!(
            said.contains("nobody has synced") && said.contains("not this Library's"),
            "and that it is two different things: {said}",
        );
    }
}
