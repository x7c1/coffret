use std::sync::Arc;

use coffret_model::MasterKey;
use coffret_usecase::ObjectStore;
use google_drive_store::{AccessTokens, DriveSettings, GoogleDrive, OAuthTokens};
use s3_store::{S3Settings, S3};

use crate::device_settings::ProviderSettings;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::{drive, s3};

/// Builds the Storage the settings describe.
pub(super) async fn build(
    dir: &LibraryDir,
    provider: &ProviderSettings,
    master_key: &MasterKey,
) -> Result<Arc<dyn ObjectStore>> {
    match provider {
        ProviderSettings::Drive {
            folder_id,
            client_id,
            client_secret,
        } => drive_store(
            dir,
            folder_id,
            client_id,
            client_secret.as_deref(),
            master_key,
        ),
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
fn drive_store(
    dir: &LibraryDir,
    folder_id: &str,
    client_id: &str,
    client_secret: Option<&str>,
    master_key: &MasterKey,
) -> Result<Arc<dyn ObjectStore>> {
    let cache = drive::token_cache(dir, master_key);

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
                cause: Some(cause),
            })
        }
    }

    let transport = drive::transport()?;
    let credentials = drive::credentials(client_id, client_secret);
    let tokens: Arc<dyn AccessTokens> =
        Arc::new(OAuthTokens::new(Arc::clone(&transport), credentials, cache));

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
