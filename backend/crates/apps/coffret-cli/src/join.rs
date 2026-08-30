//! Taking up a Library another device created.

use anyhow::bail;
use clap::{ArgGroup, Args};
use coffret_device::{join_library, JoinLibraryRequest, JoinedLibrary, JoinedProvider};

use crate::drive_client;
use crate::passphrase;
use crate::storage_location::storage;
use crate::Report;

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
    /// The Library's Recovery Code, in any grouping
    #[arg(long)]
    recovery_code: String,

    /// The Library is in Google Drive
    #[arg(long, requires = "folder_id")]
    drive: bool,
    /// The Library's own folder on Drive, by the id Drive minted for it — the
    /// one in that folder's address in Drive, and the one `init` printed
    #[arg(long, conflicts_with = "s3")]
    folder_id: Option<String>,
    /// The OAuth desktop client to authorize as; defaults to
    /// COFFRET_DRIVE_CLIENT_ID
    #[arg(long, conflicts_with = "s3")]
    client_id: Option<String>,
    /// The client secret, for a client registered with one; defaults to
    /// COFFRET_DRIVE_CLIENT_SECRET when that is set
    #[arg(long, conflicts_with = "s3")]
    client_secret: Option<String>,

    /// The Library is in an S3 bucket; the credentials are whichever the AWS
    /// SDK resolves — the environment, then a profile — and none is asked for
    /// here
    #[arg(long, requires = "bucket", requires = "prefix")]
    s3: bool,
    /// The bucket the Library is in
    #[arg(long, conflicts_with = "drive")]
    bucket: Option<String>,
    /// The Library's own prefix, ending in `coffret-<library id>/`
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
            recovery_code: args.recovery_code,
            provider,
        },
        passphrase::choosing(args.passphrase_stdin),
        |url| crate::consent::ask("join", url),
    )
    .await?;

    report(&joined);
    Ok(Report::Clean)
}

/// What the flags say about where the Library already is.
fn provider(args: &JoinArgs) -> anyhow::Result<JoinedProvider> {
    if args.drive {
        // `--drive` requires `--folder-id`, so clap has already refused the one
        // shape this could otherwise be missing.
        let Some(folder_id) = args.folder_id.clone() else {
            bail!("--drive needs --folder-id");
        };
        let (client_id, client_secret) =
            drive_client::credentials(&args.client_id, &args.client_secret)?;
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
    eprintln!(
        "\nNothing of the Library is on this device yet. Map a folder, then run \
         `coffret fetch`."
    );
}
