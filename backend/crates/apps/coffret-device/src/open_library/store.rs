use std::sync::Arc;

use coffret_model::{MasterKey, Passphrase};
use coffret_usecase::ObjectStore;
use google_drive_store::{DriveSettings, GoogleDrive};
use s3_store::{S3Settings, S3};

use crate::account_grant::{self, LibraryGrant};
use crate::accounts::Accounts;
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::reach::Reach;
use crate::{drive, s3};

/// Builds the Storage the settings describe.
///
/// `settings` is mutable because opening a Drive Library a build before
/// accounts put here promotes its grant, and records the account it went to.
pub(super) async fn build(
    reach: &Reach,
    dir: &LibraryDir,
    settings: &mut DeviceSettings,
    master_key: &MasterKey,
    passphrase: &Passphrase,
) -> Result<Arc<dyn ObjectStore>> {
    match &settings.provider {
        ProviderSettings::Drive { .. } => {
            drive_store(reach, dir, settings, master_key, passphrase).await
        }
        ProviderSettings::S3 {
            bucket,
            prefix,
            endpoint,
            region,
            path_style,
        } => Ok(s3_store(
            bucket,
            prefix,
            endpoint.as_deref(),
            region.as_deref(),
            *path_style,
        )
        .await),
    }
}

/// A store over the Library's Drive folder, if there is still a grant for it.
///
/// The grant is the account's, opened through the Library's envelope
/// (spec: SA-8, SA-9).
async fn drive_store(
    reach: &Reach,
    dir: &LibraryDir,
    settings: &mut DeviceSettings,
    master_key: &MasterKey,
    passphrase: &Passphrase,
) -> Result<Arc<dyn ObjectStore>> {
    let accounts = Accounts::open()?;
    let grant = account_grant::of_library(
        reach, dir, settings, master_key, passphrase, &accounts, None,
    )
    .await?;
    let LibraryGrant::Opened(opened) = grant else {
        return Err(Error::NotAuthorized {
            name: dir.name().to_owned(),
            cause: None,
        });
    };
    let cache = drive::token_cache(&opened.account, opened.key);

    // Asked now rather than at the first call that needs a token, because
    // "authorize again" is the answer and a person should hear it before a
    // sync has started. A cache that will not open is never read as an empty
    // one (spec: KD-10): the two are told apart in the refusal.
    match cache.load() {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(Error::NotAuthorized {
                name: dir.name().to_owned(),
                cause: None,
            })
        }
        Err(cause) => {
            return Err(Error::NotAuthorized {
                name: dir.name().to_owned(),
                cause: Some(Box::new(cause)),
            })
        }
    }

    let ProviderSettings::Drive {
        folder_id,
        client_id,
        client_secret,
        ..
    } = &settings.provider
    else {
        unreachable!("the store being built is a Drive one");
    };
    let transport = reach.drive_transport()?;
    let credentials = drive::credentials(client_id, client_secret.as_deref());
    let tokens = drive::tokens(&transport, credentials, cache);

    Ok(Arc::new(GoogleDrive::new(
        transport,
        tokens,
        DriveSettings::new(folder_id),
    )))
}

/// A store over the Library's prefix of an S3 bucket.
async fn s3_store(
    bucket: &str,
    prefix: &str,
    endpoint: Option<&str>,
    region: Option<&str>,
    path_style: bool,
) -> Arc<dyn ObjectStore> {
    let client = s3::client(endpoint, region, path_style).await;
    let settings = S3Settings::new(bucket).with_prefix(prefix);
    Arc::new(S3::new(client, settings))
}
