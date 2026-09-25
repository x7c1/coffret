use std::sync::Arc;

use coffret_format::{RecoveryCode, StoredMasterKey};
use coffret_model::{ControlObjectName, Generation, LibraryId, Passphrase};
use google_drive_store::{check_object, read_app_folder_name};
use tracing::info;
use zeroize::Zeroizing;

use super::{FoundOnStorage, JoinLibraryRequest, JoinedLibrary, JoinedProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{CreationStep, Error, Result};
use crate::reach::Reach;
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
/// starts empty — the first sync or fetch catches it up to the Library's head,
/// which is what a device that has never seen a Library does anyway
/// (spec: CK-9, RV-5). So a join that fails leaves nothing anywhere but the
/// staging directory it removes on the way out.
///
/// Where the Library lives is stated rather than searched for, and what is
/// checked is that the place given is one a Library's app folder could be: the
/// Library ID is read back out of its name, and somewhere whose name is not
/// `coffret-<library id>` is refused rather than recorded (spec: FM-18).
///
/// That much is a refusal because the name carries the identity. Whether the
/// place holds a Library is a second question and a softer one, and it is put
/// to both providers: the flow asks Storage once whether the first link of the
/// head chain is there — under the prefix on S3, in the app folder on Drive —
/// and says what it found in [`JoinedLibrary::found`]. A place holding nothing
/// is not refused: a Library created and never synced holds nothing either, and
/// the two are the same answer from here. So a caller reports it instead, and
/// somebody whose Library is going to look empty hears why now rather than
/// after a `fetch` that says nothing and succeeds. Reading keeps the promise
/// that a join writes nothing.
///
/// The two questions stay apart on Drive, where both can be asked. The name
/// decides whether this is the Library's folder at all and refuses it if not;
/// the contents decide nothing and are only reported. A folder whose name is
/// beyond doubt says nothing whatever about whether anything has been committed
/// into it.
///
/// So an app folder a user has renamed — which FM-18 leaves to them — is
/// outside what this call can take up. The Recovery Code carries no Library ID
/// (spec: KD-11), and nothing else here is told one, so the folder's name is
/// the only place it can be read from.
pub async fn join_library<R, P, F>(
    request: JoinLibraryRequest,
    enter_recovery_code: R,
    enter_passphrase: P,
    open_url: F,
) -> Result<JoinedLibrary>
where
    R: FnOnce() -> Result<Zeroizing<String>> + Send,
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    join_library_through(
        &Reach::this_device(),
        request,
        enter_recovery_code,
        enter_passphrase,
        open_url,
    )
    .await
}

/// [`join_library`], building its catalog and its Drive calls from `reach`.
///
/// The whole flow, which is what lets a case stand in for the two things it
/// reaches past this crate for and nothing else.
pub(crate) async fn join_library_through<R, P, F>(
    reach: &Reach,
    request: JoinLibraryRequest,
    enter_recovery_code: R,
    enter_passphrase: P,
    open_url: F,
) -> Result<JoinedLibrary>
where
    R: FnOnce() -> Result<Zeroizing<String>> + Send,
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    // Validate what was typed outside the secret prompts first. In particular,
    // a malformed S3 prefix leaves both lines of a script's stdin unread.
    let dir = Staging::vacant(&request.name)?;
    validate_provider(&request.provider)?;
    let settled = settled_provider(&request.provider).await?;

    // What was entered lives no longer than the parse: the block ends it either
    // way, and what carries the Master Key from here on is the parsed code.
    let code = {
        let entered = enter_recovery_code()?;
        RecoveryCode::parse(&entered).map_err(|cause| Error::MalformedRecoveryCode { cause })?
    };

    let mut staging = Staging::begin(Flow::Joining, dir)?;
    match build(
        reach,
        &request,
        &code,
        settled,
        &mut staging,
        enter_passphrase,
        open_url,
    )
    .await
    {
        Ok((settings, found)) => publish(staging, settings, found),
        Err(failure) => {
            staging.discard();
            Err(failure)
        }
    }
}

/// Refuses locally malformed provider locations before either secret is read.
fn validate_provider(provider: &JoinedProvider) -> Result<()> {
    if let JoinedProvider::S3 { prefix, .. } = provider {
        library_of_prefix(prefix)?;
    }
    Ok(())
}

/// Where the Library turns out to be, for a provider whose answer is knowable
/// before a grant exists, and what that place turned out to hold.
///
/// S3, and only S3: the prefix that was typed says which Library it is, and two
/// questions go to Storage — whether the bucket is there at all, and whether the
/// prefix holds what a Library keeps at the top of its own place. Drive is
/// asked the second of those too, but only in [`drive_folder`]: every call to
/// Drive needs a grant, and asking for one is a browser and a person.
///
/// Both questions are asked here, before either secret is read, because neither
/// needs one: nobody should type a Passphrase to be told their bucket does not
/// answer.
async fn settled_provider(
    provider: &JoinedProvider,
) -> Result<Option<(ProviderSettings, FoundOnStorage)>> {
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

    s3::check_bucket(bucket, endpoint.as_deref(), region.as_deref(), *path_style).await?;
    let found = s3::check_library_object(
        bucket,
        prefix,
        &first_head(),
        endpoint.as_deref(),
        region.as_deref(),
        *path_style,
    )
    .await?;

    Ok(Some((
        ProviderSettings::S3 {
            bucket: bucket.clone(),
            prefix: prefix.clone(),
            endpoint: endpoint.clone(),
            region: region.clone(),
            path_style: *path_style,
        },
        FoundOnStorage::of(found),
    )))
}

/// The object this flow asks a place about, to tell a place holding the Library
/// from a place holding nothing of one (spec: CP-1, FM-12, FM-13).
///
/// The first Journal record, which is written as generation 0 and states no
/// predecessor, so nothing ever supersedes it. That makes it the one object
/// every Library that has committed anything holds at the top of its own place
/// — as the Library is kept today. The name is derived rather than spelled here:
/// what a control object is called belongs to the format, and a second spelling
/// of it would be free to disagree.
///
/// What would make it the wrong object to ask about is `prune`, which is not
/// implemented (`sync` and `commit` both say so). CK-4 makes Journal records at
/// or before a Snapshot's last applied generation eligible, and CK-6 has
/// `prune` delete exactly those — generation 0 among them, from the first
/// checkpoint a Library prunes past. Such a Library holds its Journal and every
/// Entry it ever committed and does not hold this object, and a join of it would
/// be told that Storage holds nothing of the Library.
///
/// So whoever implements `prune` has to give this question something else to
/// ask, and the question stops being answerable by naming one object: which head
/// survives depends on what has been pruned, which means listing by prefix on
/// both providers.
fn first_head() -> String {
    ControlObjectName::head(Generation::FIRST).to_string()
}

/// Runs the steps, in the one order they work in.
async fn build<P, F>(
    reach: &Reach,
    request: &JoinLibraryRequest,
    code: &RecoveryCode,
    settled: Option<(ProviderSettings, FoundOnStorage)>,
    staging: &mut Staging,
    enter_passphrase: P,
    open_url: F,
) -> Result<(DeviceSettings, FoundOnStorage)>
where
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    // The Master Key first, because the token cache the Drive step writes is
    // sealed under a key derived from it (spec: KD-10). The epoch is the code's
    // own and never this build's idea of a first one.
    let key_step = |cause| staging.failed(CreationStep::StoredMasterKey, cause);
    let key_material = |cause| key_step(Error::KeyMaterial { cause });
    let passphrase = enter_passphrase()?;
    let master_key = code.master_key();
    let stored =
        StoredMasterKey::create(&passphrase, master_key, code.epoch()).map_err(key_material)?;
    StoredMasterKeyFile::write(staging.staged(), &stored).map_err(key_step)?;

    let (library_id, provider, found) = match settled {
        Some((provider, found)) => (library_of_settled(&provider)?, provider, found),
        None => drive_folder(reach, request, staging, master_key, open_url).await?,
    };

    let settings = DeviceSettings::new(library_id, provider);
    library_files::write(staging, &settings, reach)?;
    Ok((settings, found))
}

/// Asks for a grant, reads which Library the folder that was named holds, and
/// asks that folder whether anything of the Library is in it.
///
/// Two questions of one grant, in that order and never merged: the name decides
/// whether this folder is the Library's at all (spec: FM-18) and a folder that
/// is not is refused, while what the contents turn out to be decides nothing and
/// is reported. The second is what the S3 half of this flow asks its prefix, and
/// asking it here too is what makes `join` one command rather than two that
/// answer differently depending on where the Library happens to live.
///
/// Not getting an answer at all is the other thing, and it ends the join: the
/// error goes through the staging, which is then discarded, and on Drive the
/// sealed token cache lives under that staging — so the refresh token goes with
/// it and the next attempt is another browser consent. That is not a harsh
/// reading of a soft question. The answer is soft because either value is a
/// Library somebody can go on using; the call is not, because it is the same
/// `files.list` the very next `fetch` makes, so a grant that cannot answer it is
/// a grant that cannot do the Library's work either, and keeping it would only
/// move the same failure to a place with less to say about it.
async fn drive_folder<F>(
    reach: &Reach,
    request: &JoinLibraryRequest,
    staging: &mut Staging,
    master_key: &coffret_model::MasterKey,
    open_url: F,
) -> Result<(LibraryId, ProviderSettings, FoundOnStorage)>
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

    let transport = reach
        .drive_transport()
        .map_err(|cause| staging.failed(CreationStep::Authorization, cause))?;
    let (transport, tokens) = drive::grant(
        transport,
        staging.staged(),
        client_id,
        client_secret.as_deref(),
        master_key,
        open_url,
    )
    .await
    .map_err(|cause| staging.failed(CreationStep::Authorization, cause))?;

    let name = read_app_folder_name(Arc::clone(&transport), Arc::clone(&tokens), folder_id)
        .await
        .map_err(|cause| {
            staging.failed(
                CreationStep::AppFolderName,
                Error::Drive {
                    cause: Box::new(cause),
                },
            )
        })?;

    // The folder's name is the only thing that says which Library it holds, so a
    // folder called anything else is refused rather than recorded under a
    // Library ID invented here (spec: FM-18).
    let library_id = library_of_folder_name(&name)
        .map_err(|cause| staging.failed(CreationStep::AppFolderName, cause))?;

    // And now the other question, which the name cannot answer however certain
    // it is: a Library nobody has synced holds nothing in its own folder, and
    // somebody joining one is owed that word rather than an empty `fetch` they
    // have to make sense of themselves.
    let found = check_object(transport, tokens, folder_id, &first_head())
        .await
        .map_err(|cause| {
            staging.failed(
                CreationStep::LibraryObject,
                Error::Drive {
                    cause: Box::new(cause),
                },
            )
        })?;

    Ok((
        library_id,
        ProviderSettings::Drive {
            folder_id: folder_id.clone(),
            client_id: client_id.clone(),
            client_secret: client_secret.clone(),
        },
        FoundOnStorage::of(found),
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
    library_in(name).map_err(|cause| Error::NotALibraryFolder {
        location: name.to_owned(),
        cause,
    })
}

/// The Library a folder name spells, or why it spells none.
///
/// The cause without the location, so that the caller reporting a prefix and
/// the caller reporting a folder name each name what was actually handed over.
/// Apart from them rather than unwrapped from a refusal afterwards: a name is
/// refused as one thing and one thing only, and a `match` over that would carry
/// an arm nothing can ever reach.
fn library_in(name: &str) -> std::result::Result<LibraryId, Option<coffret_model::Error>> {
    let hex = name
        .strip_prefix(LibraryId::APP_FOLDER_PREFIX)
        .ok_or(None)?;
    LibraryId::from_hex(hex).map_err(Some)
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
    // Reported against what was typed rather than against the tail of it: the
    // person handed over a prefix, and that is what has to be corrected.
    library_in(name).map_err(not_one)
}

/// Moves the finished directory to the name the Library is known by.
fn publish(
    staging: Staging,
    settings: DeviceSettings,
    found: FoundOnStorage,
) -> Result<JoinedLibrary> {
    let path = staging.publish()?;

    // Worth keeping for the life of the Library: this is the moment this device
    // became one of the devices holding it. The Library ID names it on Storage
    // and is not key material, and the epoch is the one the code carried.
    info!(
        operation = OPERATION,
        library = %settings.library_id,
        provider = settings.provider.kind(),
        // Whether Storage held anything of the Library, which is the question a
        // device reporting an empty Library afterwards sends somebody looking
        // for. A verdict and not a location: nothing of the prefix is in it.
        found = matches!(found, FoundOnStorage::TheLibrary),
        "joined a Library"
    );
    Ok(JoinedLibrary {
        settings,
        path,
        found,
    })
}
