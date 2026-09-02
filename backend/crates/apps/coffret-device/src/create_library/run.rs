use coffret_format::{generate_library_id, generate_master_key, RecoveryCode, StoredMasterKey};
use coffret_model::{LibraryId, MasterKeyEpoch, Passphrase};
use google_drive_store::create_app_folder;
use tracing::info;

use super::{CreateLibraryRequest, CreatedLibrary, NewProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{CreationStep, Error, Result};
use crate::staging::{Flow, Staging};
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::{drive, library_files, s3};

/// What this call is recorded as.
const OPERATION: &str = "create_library";

/// Creates a Library and hands back the one copy of its Master Key that leaves
/// this device.
///
/// `enter_passphrase` is asked for the Passphrase the Master Key is to be stored
/// under, and it is a callback rather than a field of the request for one
/// reason: every refusal that needs no key is made before it is called. A name
/// that is not one path component, a Library of that name already on this
/// device, a base prefix that does not end in `/`, a bucket that does not answer
/// — a person hears all of those without having chosen a Passphrase twice first.
///
/// `open_url` is handed the consent URL where the provider needs a person at a
/// browser, and is never called otherwise. It is a callback for the same reason
/// the other one is: this crate has no terminal, and the same flow serves the
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
pub async fn create_library<P, F>(
    request: CreateLibraryRequest,
    enter_passphrase: P,
    open_url: F,
) -> Result<CreatedLibrary>
where
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    // Everything a refusal can be made of that needs no key, in the order it
    // costs: the name, then the place, and only then a directory. The Library ID
    // is drawn here because the S3 prefix is a function of it and nothing about
    // it comes from the Master Key (spec: FM-18).
    let dir = Staging::vacant(&request.name)?;
    let library_id = generate_library_id().map_err(|cause| Error::KeyMaterial { cause })?;
    let settled = settled_provider(&request.provider, library_id).await?;

    let mut staging = Staging::begin(Flow::Creating, dir)?;
    match build(
        &request,
        library_id,
        settled,
        &mut staging,
        enter_passphrase,
        open_url,
    )
    .await
    {
        Ok(built) => publish(staging, built),
        Err(failure) => {
            staging.discard();
            Err(failure)
        }
    }
}

/// A Library whose every file is written, in the directory it was staged in.
struct Built {
    recovery_code: RecoveryCode,
    settings: DeviceSettings,
}

/// Where the Library turns out to be, for a provider whose answer is known
/// before anything is written.
///
/// S3 is the whole of it: nothing is created there, because a prefix exists by
/// being written under, so the Library's place is settled by working out its
/// name (spec: FM-18) and the first commit is what puts anything there. The one
/// question worth putting to Storage is whether the bucket is there at all, and
/// it is put here so that the answer arrives before a Passphrase is chosen and
/// before a file exists to take back.
///
/// Drive's place is not knowable yet — the folder does not exist until a grant
/// has been given and it has been created — so it comes back `None` and is
/// settled in [`build`].
async fn settled_provider(
    provider: &NewProvider,
    library_id: LibraryId,
) -> Result<Option<ProviderSettings>> {
    let NewProvider::S3 {
        bucket,
        base_prefix,
        endpoint,
        region,
        path_style,
    } = provider
    else {
        return Ok(None);
    };

    let prefix = library_id
        .app_prefix(base_prefix)
        .map_err(|cause| Error::MalformedStoragePrefix { cause })?;
    s3::check_bucket(bucket, endpoint.as_deref(), region.as_deref(), *path_style).await?;

    Ok(Some(ProviderSettings::S3 {
        bucket: bucket.clone(),
        prefix,
        endpoint: endpoint.clone(),
        region: region.clone(),
        path_style: *path_style,
    }))
}

/// Runs the steps, in the one order they work in.
async fn build<P, F>(
    request: &CreateLibraryRequest,
    library_id: LibraryId,
    settled: Option<ProviderSettings>,
    staging: &mut Staging,
    enter_passphrase: P,
    open_url: F,
) -> Result<Built>
where
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    // The Master Key first, because the token cache the next step writes is
    // sealed under a key derived from it (spec: KD-10), and the epoch is the
    // Library's first because this is the Library coming into being.
    let epoch = MasterKeyEpoch::FIRST;
    let key_step = |cause| staging.failed(CreationStep::StoredMasterKey, cause);
    let key_material = |cause| key_step(Error::KeyMaterial { cause });
    let passphrase = enter_passphrase()?;
    let master_key = generate_master_key().map_err(key_material)?;
    let stored = StoredMasterKey::create(&passphrase, &master_key, epoch).map_err(key_material)?;
    StoredMasterKeyFile::write(staging.staged(), &stored).map_err(key_step)?;

    let provider = match settled {
        Some(provider) => provider,
        None => drive_folder(request, library_id, staging, &master_key, open_url).await?,
    };

    let settings = DeviceSettings::new(library_id, provider);
    library_files::write(staging, &settings)?;

    Ok(Built {
        recovery_code: RecoveryCode::encode(master_key, epoch),
        settings,
    })
}

/// Asks for a grant and creates the Library's folder on Drive.
async fn drive_folder<F>(
    request: &CreateLibraryRequest,
    library_id: LibraryId,
    staging: &mut Staging,
    master_key: &coffret_model::MasterKey,
    open_url: F,
) -> Result<ProviderSettings>
where
    F: FnOnce(&str) + Send,
{
    let NewProvider::Drive {
        parent,
        client_id,
        client_secret,
    } = &request.provider
    else {
        unreachable!("every provider but Drive settles its place before a file is written");
    };

    let (transport, tokens) = drive::grant(
        staging.staged(),
        client_id,
        client_secret.as_deref(),
        master_key,
        open_url,
    )
    .await
    .map_err(|cause| staging.failed(CreationStep::Authorization, cause))?;

    let folder_id = create_app_folder(transport, tokens, parent, library_id)
        .await
        .map_err(|cause| staging.failed(CreationStep::AppFolder, Error::Drive { cause }))?;
    // From here on a failure leaves a folder behind, and every refusal says so.
    staging.created_folder(folder_id.clone());

    Ok(ProviderSettings::Drive {
        folder_id,
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
    })
}

/// Moves the finished directory to the name the Library is known by.
fn publish(staging: Staging, built: Built) -> Result<CreatedLibrary> {
    let path = staging.publish()?;

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
        path,
    })
}
