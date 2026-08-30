use std::fs;
use std::sync::Arc;

use coffret_format::{generate_library_id, generate_master_key, RecoveryCode, StoredMasterKey};
use coffret_model::MasterKeyEpoch;
use coffret_sqlite_index::SqliteIndex;
use google_drive_store::{create_app_folder, AccessTokens, Authorization, OAuthTokens};
use tracing::{info, warn};

use super::{CreateLibraryRequest, CreatedLibrary, NewProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{CreationStep, Error, Result};
use crate::library_dir::LibraryDir;
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::{drive, owner_only};

/// What this call is recorded as.
const OPERATION: &str = "create_library";

/// Creates a Library and hands back the one copy of its Master Key that leaves
/// this device.
///
/// `open_url` is handed the consent URL where the provider needs a person at a
/// browser, and is never called otherwise. It is a callback rather than a
/// printed line because this crate has no terminal: the same flow serves the
/// command line today and the explorer later.
///
/// On any failure the staging directory is removed, so the device is left
/// exactly as it was. The one thing a failure can leave is an empty app folder
/// on Drive: where the create answered, the error names the folder by id, and
/// where the create itself is what failed there is no id to name — a folder
/// create is not idempotent and Drive mints the id, so an answer lost on the
/// way back may have created a folder whose id never arrived. The refusal says
/// so, because a second `init` that does not look first leaves two `coffret-`
/// folders where discovery by enumeration expects one (spec: FM-18).
pub async fn create_library<F>(request: CreateLibraryRequest, open_url: F) -> Result<CreatedLibrary>
where
    F: FnOnce(&str) + Send,
{
    let dir = LibraryDir::resolve(&request.name)?;
    if dir.is_present() {
        return Err(Error::LibraryExists {
            name: dir.name().to_owned(),
            path: dir.path().to_path_buf(),
        });
    }

    let staging = dir.staging();
    if staging.path().exists() {
        // Discarded rather than resumed: nothing in it reached Storage under a
        // key anything kept, so there is no state in it worth more than the
        // certainty of starting from nothing.
        fs::remove_dir_all(staging.path()).map_err(Error::local(
            "discarding what an interrupted creation left",
            staging.path(),
        ))?;
        info!(
            operation = OPERATION,
            "discarded a directory an interrupted creation left"
        );
    }
    owner_only::create_dir("creating the Library directory", staging.path())?;

    let mut attempt = Attempt {
        name: request.name.clone(),
        folder: None,
    };
    match build(&request, &staging, &mut attempt, open_url).await {
        Ok(built) => publish(&dir, &staging, &attempt, built),
        Err(failure) => {
            discard(&staging);
            Err(failure)
        }
    }
}

/// What one creation has done so far that a failure has to report.
struct Attempt {
    name: String,
    folder: Option<String>,
}

impl Attempt {
    /// Reports the step that failed, and the folder this attempt cannot take
    /// back.
    fn failed(&self, step: CreationStep, cause: Error) -> Error {
        Error::LibraryNotCreated {
            name: self.name.clone(),
            step,
            orphan_folder: self.folder.clone(),
            cause: Box::new(cause),
        }
    }
}

/// A Library whose every file is written, in the directory it was staged in.
struct Built {
    recovery_code: RecoveryCode,
    settings: DeviceSettings,
}

/// Runs the steps, in the one order they work in.
async fn build<F>(
    request: &CreateLibraryRequest,
    staging: &LibraryDir,
    attempt: &mut Attempt,
    open_url: F,
) -> Result<Built>
where
    F: FnOnce(&str) + Send,
{
    // The Master Key first, because the token cache the next step writes is
    // sealed under a key derived from it (spec: KD-10), and the epoch is the
    // Library's first because this is the Library coming into being.
    let epoch = MasterKeyEpoch::FIRST;
    let key_step = |cause| attempt.failed(CreationStep::StoredMasterKey, cause);
    let key_material = |cause| key_step(Error::KeyMaterial { cause });
    let master_key = generate_master_key().map_err(key_material)?;
    let library_id = generate_library_id().map_err(key_material)?;
    let stored =
        StoredMasterKey::create(&request.passphrase, &master_key, epoch).map_err(key_material)?;
    StoredMasterKeyFile::write(staging, &stored).map_err(key_step)?;

    let provider = match &request.provider {
        NewProvider::Drive {
            parent,
            client_id,
            client_secret,
        } => {
            let transport = drive::transport()
                .map_err(|cause| attempt.failed(CreationStep::Authorization, cause))?;
            let credentials = drive::credentials(client_id, client_secret.as_deref());
            let cache = drive::token_cache(staging, master_key.clone());

            Authorization::new(Arc::clone(&transport), credentials.clone(), cache.clone())
                .run(open_url)
                .await
                .map_err(|cause| {
                    attempt.failed(CreationStep::Authorization, Error::Drive { cause })
                })?;

            let tokens: Arc<dyn AccessTokens> =
                Arc::new(OAuthTokens::new(Arc::clone(&transport), credentials, cache));
            let folder_id = create_app_folder(transport, tokens, parent.as_deref(), library_id)
                .await
                .map_err(|cause| attempt.failed(CreationStep::AppFolder, Error::Drive { cause }))?;
            // From here on a failure leaves a folder behind, and every refusal
            // says so.
            attempt.folder = Some(folder_id.clone());

            ProviderSettings::Drive {
                folder_id,
                client_id: client_id.clone(),
                client_secret: client_secret.clone(),
            }
        }
        NewProvider::S3 {
            bucket,
            base_prefix,
            endpoint,
            region,
            path_style,
        } => {
            // Nothing is created: on S3 a prefix exists by being written under,
            // so the Library's place is settled by working out its name
            // (spec: FM-18) and the first commit is what puts anything there.
            let prefix = library_id.app_prefix(base_prefix).map_err(|cause| {
                attempt.failed(
                    CreationStep::StoragePrefix,
                    Error::MalformedStoragePrefix { cause },
                )
            })?;
            ProviderSettings::S3 {
                bucket: bucket.clone(),
                prefix,
                endpoint: endpoint.clone(),
                region: region.clone(),
                path_style: *path_style,
            }
        }
    };

    // Created empty and owner-only first, then handed to SQLite: the catalog is
    // plaintext and names Entry Paths, so it must never exist at whatever mode
    // the process umask would have given it, not even for an instant.
    let index_file = staging.index_file();
    owner_only::create_empty_file("creating the catalog", &index_file)
        .map_err(|cause| attempt.failed(CreationStep::Index, cause))?;
    SqliteIndex::open(&index_file)
        .map_err(|cause| attempt.failed(CreationStep::Index, Error::Index { cause }))?;

    owner_only::create_dir("creating the spool directory", &staging.spool_dir())
        .map_err(|cause| attempt.failed(CreationStep::Spool, cause))?;

    // Last, because a directory carrying one is a Library anything may open.
    let settings = DeviceSettings::new(library_id, provider);
    settings
        .write(staging)
        .map_err(|cause| attempt.failed(CreationStep::Settings, cause))?;

    Ok(Built {
        recovery_code: RecoveryCode::encode(&master_key, epoch),
        settings,
    })
}

/// Moves the finished directory to the name the Library is known by.
fn publish(
    dir: &LibraryDir,
    staging: &LibraryDir,
    attempt: &Attempt,
    built: Built,
) -> Result<CreatedLibrary> {
    if let Err(cause) = fs::rename(staging.path(), dir.path()) {
        let failure = attempt.failed(
            CreationStep::Publish,
            Error::Local {
                doing: "moving the finished Library directory into place",
                path: dir.path().to_path_buf(),
                cause,
            },
        );
        discard(staging);
        return Err(failure);
    }

    // Worth keeping for the life of the Library: this is the moment it came
    // into being. The Library ID names it on Storage and is not key material,
    // and the provider is a constant — nothing here is a path or a secret.
    info!(
        operation = OPERATION,
        library = %built.settings.library_id,
        provider = built.settings.provider.kind(),
        "created a Library"
    );
    Ok(CreatedLibrary {
        recovery_code: built.recovery_code,
        settings: built.settings,
        path: dir.path().to_path_buf(),
    })
}

/// Removes what a creation that did not finish had built so far.
fn discard(staging: &LibraryDir) {
    if let Err(cause) = fs::remove_dir_all(staging.path()) {
        // There is nothing to do about it and nothing that depends on it — the
        // Library was not created either way — so it is recorded rather than
        // reported over the failure that actually stopped the creation.
        warn!(
            operation = OPERATION,
            reason = %cause,
            "could not remove what an interrupted creation left"
        );
    }
}
