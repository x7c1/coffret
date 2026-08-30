use coffret_format::{RecoveryCode, StoredMasterKey};
use coffret_model::LibraryId;
use google_drive_store::read_app_folder_name;
use tracing::info;

use super::{JoinLibraryRequest, JoinedLibrary, JoinedProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{CreationStep, Error, Result};
use crate::staging::{Flow, Staging};
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::{drive, library_files, s3};

/// What this call is recorded as.
const OPERATION: &str = "join_library";

/// Takes up a Library another device created, from its Recovery Code.
///
/// The sibling of [`create_library`](crate::create_library), and the same five
/// things come out of it: a directory holding the Master Key under this device's
/// own Passphrase, a catalog, a spool, and a note of where the Library's Storage
/// is. Two things differ, and both follow from the Library already existing.
///
/// The Master Key is *entered* rather than drawn. It comes out of the Recovery
/// Code with the epoch it was written at, and that epoch is what the stored form
/// records: a code written before a rotation carries the key of the epoch it
/// belongs to, and claiming a later one would have this device deriving the
/// wrong keys for everything it read (spec: KD-11, KD-9).
///
/// And nothing is written to Storage. The app folder is already the Library's,
/// the Keyring and the Journal are already there, and this device's catalog
/// starts empty — the first sync or fetch catches it up from the Library's head,
/// which is what a device that has never seen a Library does anyway
/// (spec: CK-9, RV-1). So a join that fails leaves nothing anywhere but the
/// staging directory it removes on the way out.
///
/// Where the Library lives is stated rather than searched for, and what is
/// checked is that the place given is one a Library's app folder could be: the
/// Library ID is read back out of its name, and somewhere whose name is not
/// `coffret-<library id>` is refused rather than recorded (spec: FM-18).
///
/// So an app folder a user has renamed — which FM-18 leaves to them — is
/// outside what this call can take up. The Recovery Code carries no Library ID
/// (spec: KD-11), and nothing else here is told one, so the folder's name is
/// the only place it can be read from.
pub async fn join_library<P, F>(
    request: JoinLibraryRequest,
    enter_passphrase: P,
    open_url: F,
) -> Result<JoinedLibrary>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
    F: FnOnce(&str) + Send,
{
    // Every refusal that needs no Passphrase, in the order it costs: the name,
    // then the code, then the place. A code that is not one is refused here and
    // releases no key material either way (spec: KD-11).
    let dir = Staging::vacant(&request.name)?;
    let code = RecoveryCode::parse(&request.recovery_code)
        .map_err(|cause| Error::MalformedRecoveryCode { cause })?;
    let settled = settled_provider(&request.provider).await?;

    let mut staging = Staging::begin(Flow::Joining, dir)?;
    match build(
        &request,
        &code,
        settled,
        &mut staging,
        enter_passphrase,
        open_url,
    )
    .await
    {
        Ok(settings) => publish(staging, settings),
        Err(failure) => {
            staging.discard();
            Err(failure)
        }
    }
}

/// Where the Library turns out to be, for a provider whose answer is knowable
/// before a grant exists.
///
/// S3, and only S3: the prefix that was typed says which Library it is, and the
/// one question worth putting to Storage is whether the bucket is there at all.
/// Drive's answer is a call that needs a grant, so it waits for [`build`].
async fn settled_provider(provider: &JoinedProvider) -> Result<Option<ProviderSettings>> {
    let JoinedProvider::S3 {
        bucket,
        prefix,
        endpoint,
        region,
        path_style,
    } = provider
    else {
        return Ok(None);
    };

    // Read rather than trusted: the prefix is what the person typed, and a
    // Library ID this device recorded wrongly would name a folder nothing else
    // is configured against (spec: FM-18).
    library_of_prefix(prefix)?;
    s3::check_bucket(bucket, endpoint.as_deref(), region.as_deref(), *path_style).await?;

    Ok(Some(ProviderSettings::S3 {
        bucket: bucket.clone(),
        prefix: prefix.clone(),
        endpoint: endpoint.clone(),
        region: region.clone(),
        path_style: *path_style,
    }))
}

/// Runs the steps, in the one order they work in.
async fn build<P, F>(
    request: &JoinLibraryRequest,
    code: &RecoveryCode,
    settled: Option<ProviderSettings>,
    staging: &mut Staging,
    enter_passphrase: P,
    open_url: F,
) -> Result<DeviceSettings>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
    F: FnOnce(&str) + Send,
{
    // The Master Key first, because the token cache the Drive step writes is
    // sealed under a key derived from it (spec: KD-10). The epoch is the code's
    // own and never this build's idea of a first one.
    let key_step = |cause| staging.failed(CreationStep::StoredMasterKey, cause);
    let key_material = |cause| key_step(Error::KeyMaterial { cause });
    let passphrase = enter_passphrase()?;
    let master_key = code.master_key().clone();
    let stored =
        StoredMasterKey::create(&passphrase, &master_key, code.epoch()).map_err(key_material)?;
    StoredMasterKeyFile::write(staging.staged(), &stored).map_err(key_step)?;

    let (library_id, provider) = match settled {
        Some(provider) => (library_of_settled(&provider)?, provider),
        None => drive_folder(request, staging, &master_key, open_url).await?,
    };

    let settings = DeviceSettings::new(library_id, provider);
    library_files::write(staging, &settings)?;
    Ok(settings)
}

/// Asks for a grant and reads which Library the folder that was named holds.
async fn drive_folder<F>(
    request: &JoinLibraryRequest,
    staging: &mut Staging,
    master_key: &coffret_model::MasterKey,
    open_url: F,
) -> Result<(LibraryId, ProviderSettings)>
where
    F: FnOnce(&str) + Send,
{
    let JoinedProvider::Drive {
        folder_id,
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

    let name = read_app_folder_name(transport, tokens, folder_id)
        .await
        .map_err(|cause| staging.failed(CreationStep::AppFolderName, Error::Drive { cause }))?;

    // The folder's name is the only thing that says which Library it holds, so a
    // folder called anything else is refused rather than recorded under a
    // Library ID invented here (spec: FM-18).
    let library_id = library_of_folder_name(&name)
        .map_err(|cause| staging.failed(CreationStep::AppFolderName, cause))?;

    Ok((
        library_id,
        ProviderSettings::Drive {
            folder_id: folder_id.clone(),
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
        },
    ))
}

/// The Library a settled provider's place names.
fn library_of_settled(provider: &ProviderSettings) -> Result<LibraryId> {
    match provider {
        ProviderSettings::S3 { prefix, .. } => library_of_prefix(prefix),
        ProviderSettings::Drive { folder_id, .. } => {
            unreachable!("a Drive Library at {folder_id:?} settles its place with a call")
        }
    }
}

/// The Library the app folder called `name` holds (spec: FM-18).
pub(super) fn library_of_folder_name(name: &str) -> Result<LibraryId> {
    let not_one = |cause| Error::NotALibraryFolder {
        location: name.to_owned(),
        cause,
    };

    let hex = name
        .strip_prefix(LibraryId::APP_FOLDER_PREFIX)
        .ok_or_else(|| not_one(None))?;
    LibraryId::from_hex(hex).map_err(|cause| not_one(Some(cause)))
}

/// The Library the key prefix `prefix` holds (spec: FM-18).
///
/// The prefix is the app folder's name as a key prefix, so it is the folder name
/// with a `/` after it — and the `/` is required rather than forgiven: a prefix
/// without it names keys starting with the folder's name rather than keys inside
/// it, which is a different place and one nothing else would look in.
pub(super) fn library_of_prefix(prefix: &str) -> Result<LibraryId> {
    let not_one = |cause| Error::NotALibraryFolder {
        location: prefix.to_owned(),
        cause,
    };

    let folder = prefix.strip_suffix('/').ok_or_else(|| not_one(None))?;
    let name = folder.rsplit('/').next().unwrap_or(folder);
    library_of_folder_name(name).map_err(|error| match error {
        // Reported against what was typed rather than against the tail of it:
        // the person handed over a prefix, and that is what has to be corrected.
        Error::NotALibraryFolder { cause, .. } => not_one(cause),
        other => other,
    })
}

/// Moves the finished directory to the name the Library is known by.
fn publish(staging: Staging, settings: DeviceSettings) -> Result<JoinedLibrary> {
    let path = staging.publish()?;

    // Worth keeping for the life of the Library: this is the moment this device
    // became one of the devices holding it. The Library ID names it on Storage
    // and is not key material, and the epoch is the one the code carried.
    info!(
        operation = OPERATION,
        library = %settings.library_id,
        provider = settings.provider.kind(),
        "joined a Library"
    );
    Ok(JoinedLibrary { settings, path })
}
