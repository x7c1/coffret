use anyhow::{bail, Context};
use clap::{ArgGroup, Args};
use coffret_device::{
    create_library, CreateLibraryRequest, CreatedLibrary, NewProvider, ProviderSettings,
};

use crate::passphrase;
use crate::recovery_code::print_recovery_code;

/// Where the OAuth client id comes from when it is not typed.
///
/// There is no client id built into this binary. Registering one is the
/// account owner's to do, and a shared one would put every user of coffret in
/// the same consent screen quota.
const CLIENT_ID: &str = "COFFRET_DRIVE_CLIENT_ID";

/// Exactly one provider, and only the flags that provider has.
///
/// A flag the chosen provider knows nothing about is refused rather than
/// ignored: `--endpoint` with `--drive` would otherwise look like it had been
/// applied, and the person would find out where their Library really went the
/// next time they looked for it.
#[derive(Args)]
#[command(group(ArgGroup::new("provider").required(true).args(["drive", "s3"])))]
pub struct InitArgs {
    /// What this device is to call the Library
    #[arg(long)]
    name: String,

    /// Keep the Library in Google Drive
    #[arg(long)]
    drive: bool,
    /// The Drive folder to create the Library's own folder in, if not the top
    /// of My Drive
    #[arg(long, conflicts_with = "s3")]
    parent: Option<String>,
    /// The OAuth desktop client to authorize as; defaults to
    /// COFFRET_DRIVE_CLIENT_ID
    #[arg(long, conflicts_with = "s3")]
    client_id: Option<String>,
    /// The client secret, for a client registered with one
    #[arg(long, conflicts_with = "s3")]
    client_secret: Option<String>,

    /// Keep the Library in an S3 bucket; the credentials are whichever the AWS
    /// SDK resolves — the environment, then a profile — and none is asked for
    /// here
    #[arg(long, requires = "bucket")]
    s3: bool,
    /// The bucket to keep the Library in
    #[arg(long, conflicts_with = "drive")]
    bucket: Option<String>,
    /// Where in the bucket to put it; a prefix ending in `/`, or the bucket
    /// root when it is not given
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

pub async fn run(args: InitArgs) -> anyhow::Result<()> {
    let provider = provider(&args)?;
    let passphrase = passphrase::choose(args.passphrase_stdin)?;

    let created = create_library(
        CreateLibraryRequest {
            name: args.name,
            passphrase,
            provider,
        },
        |url| crate::consent::ask("init", url),
    )
    .await?;

    report(&created);
    Ok(())
}

/// What the flags say about where the Library is to live.
fn provider(args: &InitArgs) -> anyhow::Result<NewProvider> {
    if args.drive {
        let client_id = match &args.client_id {
            Some(client_id) => client_id.clone(),
            None => std::env::var(CLIENT_ID)
                .with_context(|| format!("--client-id was not given and {CLIENT_ID} is not set"))?,
        };
        return Ok(NewProvider::Drive {
            parent: args.parent.clone(),
            client_id,
            client_secret: args.client_secret.clone(),
        });
    }

    // `--s3` requires `--bucket`, so clap has already refused the one shape
    // this could otherwise be missing.
    let Some(bucket) = args.bucket.clone() else {
        bail!("--s3 needs --bucket");
    };
    Ok(NewProvider::S3 {
        bucket,
        base_prefix: args.prefix.clone().unwrap_or_default(),
        endpoint: args.endpoint.clone(),
        region: args.region.clone(),
        path_style: args.path_style,
    })
}

/// Says what was created, and what the person now has to do about it.
fn report(created: &CreatedLibrary) {
    eprintln!("\nThe Library is at {}.", created.path.display());
    eprintln!("Library ID: {}", created.settings.library_id);
    eprintln!("On Storage: {}", storage(&created.settings.provider));
    print_recovery_code(&created.recovery_code);
}

/// Where the Library's objects go, in the words the provider uses for it.
///
/// Said back because `init` promises a Library "on this device and on Storage"
/// and nothing else in the report covers the second half. It matters most on
/// S3, where creating a Library reaches no network at all: a bucket or a prefix
/// that is not the one that was meant would otherwise go unnoticed until the
/// first upload.
fn storage(provider: &ProviderSettings) -> String {
    match provider {
        ProviderSettings::Drive { folder_id, .. } => {
            format!("the Google Drive folder {folder_id}")
        }
        ProviderSettings::S3 { bucket, prefix, .. } => format!("s3://{bucket}/{prefix}"),
    }
}
