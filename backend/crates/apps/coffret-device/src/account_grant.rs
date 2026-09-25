//! How a Library reaches the grant of the account it references: set up for a
//! Library being put on this device, opened for one already here, and promoted
//! from a Library's own previous grant (spec: SA-8, SA-9).

use std::fs;
use std::io::ErrorKind;
use std::sync::Arc;

use coffret_format::generate_account_cache_key;
use coffret_model::{AccountCacheKey, MasterKey, Passphrase, Redacted};
use coffret_usecase::{LocalIoError, LocalOperation};
use google_drive_store::{
    read_app_folder_name, AccessTokens, ClientCredentials, HttpTransport, TokenCache,
};
use tracing::{debug, info};

use crate::account_dir::AccountDir;
use crate::account_envelope::AccountEnvelope;
use crate::account_name::AccountName;
use crate::account_settings::AccountSettings;
use crate::accounts::{Accounts, Choice, NewAccount};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::drive;
use crate::error::{Error, PromotionObstacle, Result};
use crate::library_dir::LibraryDir;
use crate::reach::Reach;
use crate::referencing_passphrase::ReferencingPassphrase;

/// What this is recorded as.
const OPERATION: &str = "account_grant";

/// The grant of the account a Library being put on this device references.
pub(crate) struct Bound {
    /// The account the Library is to reference.
    pub(crate) account: AccountName,
    /// That account's account-cache key, for the Library's envelope.
    pub(crate) key: Arc<AccountCacheKey>,
    /// What a call to Drive is made with, minted from the account's cache.
    pub(crate) tokens: Arc<dyn AccessTokens>,
    /// The account, where this Library is the one bringing it to the device;
    /// published once the Library is in place (see [`NewAccount`]).
    pub(crate) new_account: Option<NewAccount>,
}

/// Sets up the grant of a Library being put on this device, on the account
/// `choice` names.
///
/// An account the device holds is reached through a Library already
/// referencing it — the one place its key is kept (spec: SA-9) — unlocked with
/// `passphrase`, or failing that with the Passphrase `referencing` is asked for,
/// and costs no consent while its cache holds a grant. A new one draws its own
/// account-cache key and is consented to (spec: KD-12, SA-4).
pub(crate) async fn chosen<F>(
    transport: &Arc<dyn HttpTransport>,
    accounts: &Accounts,
    choice: Choice,
    passphrase: &Passphrase,
    referencing: &ReferencingPassphrase,
    credentials: ClientCredentials,
    open_url: F,
) -> Result<Bound>
where
    F: FnOnce(&str) + Send,
{
    match choice {
        Choice::Held(dir) => {
            let key = Arc::new(accounts.key_for(dir.name(), passphrase, referencing)?);
            let cache = drive::token_cache(&dir, Arc::clone(&key));
            if cache.load()?.is_none() {
                // The account is here and its grant is not: it is renewed here
                // rather than left for the next run to find missing.
                drive::consent(transport, credentials.clone(), cache.clone(), open_url).await?;
            }
            Ok(Bound {
                account: dir.name().clone(),
                key,
                tokens: drive::tokens(transport, credentials, cache),
                new_account: None,
            })
        }
        Choice::New(name) => {
            let key = Arc::new(
                generate_account_cache_key().map_err(|cause| Error::KeyMaterial { cause })?,
            );
            let settings =
                AccountSettings::new(credentials.client_id(), credentials.client_secret());
            let new_account = NewAccount::begin(&name, &settings)?;
            let cache = drive::token_cache(new_account.staged(), Arc::clone(&key));
            drive::consent(transport, credentials.clone(), cache.clone(), open_url).await?;
            Ok(Bound {
                account: name,
                key,
                tokens: drive::tokens(transport, credentials, cache),
                new_account: Some(new_account),
            })
        }
    }
}

/// The account this device holds whose Drive lists the app folder `folder_id`,
/// and the folder's name as that account reads it (spec: SA-8).
///
/// Every account consented to through `credentials`' client is tried — one
/// consented to another client is another grant with another reach — and one
/// whose cache holds no grant, or whose Drive does not answer for the folder,
/// is passed over. `None` where none of them reaches it, which is when a join
/// asks for a consent.
///
/// The order is the point. Asking an account's Drive takes opening its cache,
/// and opening its cache takes the Passphrase of a Library that references it.
/// The accounts the new Library's own Passphrase opens cost nobody anything, so
/// every one of them is asked first; only where none of them reaches the folder
/// is a person asked for another Library's Passphrase — one account at a time,
/// stopping at the first that reaches it — so a device kept under one
/// Passphrase asks nothing and one that is not asks as little as it can. An
/// account that cannot be opened at all, with nobody to ask or with a
/// Passphrase that does not open the Library named, ends the join: consenting
/// past it could keep a second cache for an account the device already holds.
pub(crate) async fn search(
    transport: &Arc<dyn HttpTransport>,
    accounts: &Accounts,
    passphrase: &Passphrase,
    referencing: &ReferencingPassphrase,
    credentials: &ClientCredentials,
    folder_id: &str,
) -> Result<Option<(Bound, String)>> {
    let mut unopened = Vec::new();
    for dir in accounts.held() {
        let same_client = AccountSettings::read(dir)?
            .is_some_and(|settings| settings.client_id == credentials.client_id());
        if !same_client {
            continue;
        }
        match accounts.key_through(dir.name(), passphrase) {
            Some(key) => {
                if let Some(found) = probe(transport, dir, key, credentials, folder_id).await {
                    return Ok(Some(found));
                }
            }
            None => unopened.push(dir),
        }
    }
    for dir in unopened {
        let key = accounts.key_for(dir.name(), passphrase, referencing)?;
        if let Some(found) = probe(transport, dir, key, credentials, folder_id).await {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The grant of `account`, where its Drive lists the app folder `folder_id`,
/// and the folder's name as it reads it.
async fn probe(
    transport: &Arc<dyn HttpTransport>,
    account: &AccountDir,
    key: AccountCacheKey,
    credentials: &ClientCredentials,
    folder_id: &str,
) -> Option<(Bound, String)> {
    let key = Arc::new(key);
    let cache = drive::token_cache(account, Arc::clone(&key));
    if !matches!(cache.load(), Ok(Some(_))) {
        return None;
    }
    let tokens = drive::tokens(transport, credentials.clone(), cache);
    match read_app_folder_name(Arc::clone(transport), Arc::clone(&tokens), folder_id).await {
        Ok(name) => Some((
            Bound {
                account: account.name().clone(),
                key,
                tokens,
                new_account: None,
            },
            name,
        )),
        // The gateway records what Drive said where it read it; what is kept
        // here is that this account was passed over.
        Err(cause) => {
            debug!(
                operation = OPERATION,
                reason = %Error::from(cause).redacted(),
                "an account this device holds does not reach the folder a join named"
            );
            None
        }
    }
}

/// The account a Library already on this device references, opened.
pub(crate) struct Opened {
    /// Where the account's grant is kept.
    pub(crate) account: AccountDir,
    /// The account-cache key the Library's envelope yielded.
    pub(crate) key: Arc<AccountCacheKey>,
}

/// What a Library on this device turned out to reference.
pub(crate) enum LibraryGrant {
    /// An account, opened through the Library's envelope.
    Opened(Opened),
    /// No account, and no grant of its own to promote into one.
    Unbound,
}

/// Opens the account a Drive Library references, promoting the Library's own
/// previous grant into one first where that is what it holds (spec: SA-8,
/// SA-9).
///
/// `requested` is an account the person named for this Library. It is where a
/// promotion goes, and a Library that already references another is refused
/// rather than moved: the name is bound into its envelope.
pub(crate) async fn of_library(
    reach: &Reach,
    dir: &LibraryDir,
    settings: &mut DeviceSettings,
    master_key: &MasterKey,
    passphrase: &Passphrase,
    accounts: &Accounts,
    requested: Option<&AccountName>,
) -> Result<LibraryGrant> {
    let named = named_account(settings)?;
    let previous = dir.previous_token_cache_file().is_file();

    // The one path by which a Library leaves its previous shape: its own cache
    // and no envelope, or an envelope an interrupted promotion wrote before the
    // settings came to name the account (spec: SA-8).
    if previous && (named.is_none() || !AccountEnvelope::is_present(dir)) {
        let target = requested
            .cloned()
            .or_else(|| named.clone())
            .unwrap_or_else(AccountName::default_name);
        return promote(
            reach, dir, settings, master_key, passphrase, accounts, target,
        )
        .await;
    }

    let Some(account) = named else {
        return Ok(LibraryGrant::Unbound);
    };
    if let Some(requested) = requested.filter(|requested| **requested != account) {
        return Err(Error::AccountFixed {
            library: dir.name().to_owned(),
            account: account.as_str().to_owned(),
            requested: requested.as_str().to_owned(),
        });
    }

    let key = Arc::new(AccountEnvelope::open(dir, master_key, &account)?);
    let account_dir = AccountDir::resolve(&account)?;
    Accounts::require_client(&account_dir, dir.name(), drive_client(settings).0)?;
    if previous {
        complete_promotion(dir, settings, master_key, &account_dir, &key)?;
    }
    Ok(LibraryGrant::Opened(Opened {
        account: account_dir,
        key,
    }))
}

/// Brings a Library that references no account onto one, for a renewal that is
/// about to consent for it.
///
/// The choice `init` makes, and the envelope and the settings written before
/// the grant is: the account then has a Library naming it from the moment it
/// exists.
pub(crate) fn bind(
    dir: &LibraryDir,
    settings: &mut DeviceSettings,
    master_key: &MasterKey,
    passphrase: &Passphrase,
    accounts: &Accounts,
    requested: Option<AccountName>,
) -> Result<Opened> {
    let (client_id, _) = drive_client(settings);
    let (account, key) = match accounts.choose(requested)? {
        Choice::Held(held) => {
            Accounts::require_client(&held, dir.name(), client_id)?;
            let key =
                accounts.key_for(held.name(), passphrase, &ReferencingPassphrase::unasked())?;
            (held, key)
        }
        Choice::New(name) => {
            let key = generate_account_cache_key().map_err(|cause| Error::KeyMaterial { cause })?;
            (AccountDir::resolve(&name)?, key)
        }
    };
    AccountEnvelope::write(dir, master_key, account.name(), &key)?;
    record_account(dir, settings, account.name())?;
    Ok(Opened {
        account,
        key: Arc::new(key),
    })
}

/// Moves a Library's own previous grant into the account `target`
/// (spec: SA-8).
///
/// Into a new account under a fresh account-cache key, or — where the device
/// already holds `target` — into nothing at all: the Library references that
/// account if its grant reaches the Library's folder, the choice a join makes,
/// and its own cache is removed unused. Where it does not, or cannot be opened,
/// the promotion stops and asks for a name.
///
/// The order is what makes an interruption harmless: the account and the
/// envelope first, the settings naming the account next, and the Library's own
/// cache last, so a promotion stopped anywhere leaves the previous shape to
/// promote again.
async fn promote(
    reach: &Reach,
    dir: &LibraryDir,
    settings: &mut DeviceSettings,
    master_key: &MasterKey,
    passphrase: &Passphrase,
    accounts: &Accounts,
    target: AccountName,
) -> Result<LibraryGrant> {
    let tokens = match drive::previous_token_cache(dir, master_key).load() {
        Ok(Some(tokens)) => tokens,
        // Gone between the look and the read: another run promoted it.
        Ok(None) => return Ok(LibraryGrant::Unbound),
        // Never read as an empty cache (spec: KD-10).
        Err(cause) => {
            return Err(Error::NotAuthorized {
                name: dir.name().to_owned(),
                cause: Some(Box::new(cause)),
            })
        }
    };
    let (client_id, client_secret) = drive_client(settings);

    if let Some(held) = accounts.get(&target) {
        let needs_name = |obstacle| Error::PromotionNeedsName {
            library: dir.name().to_owned(),
            account: target.as_str().to_owned(),
            obstacle,
        };
        // Only another client asks for another name; a settings file that
        // cannot be read is reported as itself.
        match Accounts::require_client(held, dir.name(), client_id) {
            Ok(()) => {}
            Err(Error::ClientMismatch(mismatch)) => {
                return Err(needs_name(PromotionObstacle::ClientDiffers(mismatch)))
            }
            Err(other) => return Err(other),
        }
        let key = Arc::new(
            accounts
                .key_through(&target, passphrase)
                .ok_or_else(|| needs_name(PromotionObstacle::NotOpened))?,
        );
        if !reaches_folder(reach, held, &key, settings).await? {
            return Err(needs_name(PromotionObstacle::FolderNotReached));
        }
        AccountEnvelope::write(dir, master_key, &target, &key)?;
        record_account(dir, settings, &target)?;
        remove_previous(dir)?;
        info!(
            operation = OPERATION,
            into = "held",
            "promoted a Library's own grant: it references an account this device held"
        );
        return Ok(LibraryGrant::Opened(Opened {
            account: held.clone(),
            key,
        }));
    }

    let key = Arc::new(generate_account_cache_key().map_err(|cause| Error::KeyMaterial { cause })?);
    let new_account = NewAccount::begin(&target, &AccountSettings::new(client_id, client_secret))?;
    TokenCache::for_account(new_account.staged().token_cache_file(), Arc::clone(&key))
        .store(&tokens)?;
    AccountEnvelope::write(dir, master_key, &target, &key)?;
    record_account(dir, settings, &target)?;
    let account = new_account.publish()?;
    remove_previous(dir)?;
    info!(
        operation = OPERATION,
        into = "new",
        "promoted a Library's own grant into a new account's cache"
    );
    Ok(LibraryGrant::Opened(Opened { account, key }))
}

/// Whether the grant of `account` reaches the Library's app folder.
async fn reaches_folder(
    reach: &Reach,
    account: &AccountDir,
    key: &Arc<AccountCacheKey>,
    settings: &DeviceSettings,
) -> Result<bool> {
    let ProviderSettings::Drive {
        folder_id,
        client_id,
        client_secret,
        ..
    } = &settings.provider
    else {
        return Ok(false);
    };
    let cache = drive::token_cache(account, Arc::clone(key));
    if !matches!(cache.load(), Ok(Some(_))) {
        return Ok(false);
    }
    let transport = reach.drive_transport()?;
    let tokens = drive::tokens(
        &transport,
        drive::credentials(client_id, client_secret.as_deref()),
        cache,
    );
    Ok(read_app_folder_name(transport, tokens, folder_id)
        .await
        .is_ok())
}

/// Finishes a promotion that stopped after the settings came to name the
/// account and before the Library's own cache was removed.
///
/// Where the account's cache is there, the Library's own is what the promotion
/// had left to remove. Where it is not — the account had not yet taken its name
/// — the Library's own grant is what fills it, under the key its envelope
/// already holds.
fn complete_promotion(
    dir: &LibraryDir,
    settings: &DeviceSettings,
    master_key: &MasterKey,
    account: &AccountDir,
    key: &Arc<AccountCacheKey>,
) -> Result<()> {
    if !account.token_cache_file().is_file() {
        let Ok(Some(tokens)) = drive::previous_token_cache(dir, master_key).load() else {
            return Ok(());
        };
        if !account.is_present() {
            let (client_id, client_secret) = drive_client(settings);
            AccountSettings::new(client_id, client_secret).write(account)?;
        }
        drive::token_cache(account, Arc::clone(key)).store(&tokens)?;
    }
    remove_previous(dir)
}

/// The account a Library's settings name, if any.
fn named_account(settings: &DeviceSettings) -> Result<Option<AccountName>> {
    match &settings.provider {
        ProviderSettings::Drive {
            account: Some(account),
            ..
        } => AccountName::parse(account).map(Some),
        _ => Ok(None),
    }
}

/// The client a Drive Library's settings name, and its secret.
fn drive_client(settings: &DeviceSettings) -> (&str, Option<&str>) {
    match &settings.provider {
        ProviderSettings::Drive {
            client_id,
            client_secret,
            ..
        } => (client_id, client_secret.as_deref()),
        ProviderSettings::S3 { .. } => unreachable!("only a Drive Library references an account"),
    }
}

/// Records in the Library's settings that it references `account`.
fn record_account(
    dir: &LibraryDir,
    settings: &mut DeviceSettings,
    account: &AccountName,
) -> Result<()> {
    if let ProviderSettings::Drive {
        account: recorded, ..
    } = &mut settings.provider
    {
        *recorded = Some(account.as_str().to_owned());
    }
    settings.write(dir)
}

/// Removes a Library's own previous grant, which its account's cache now holds
/// or never needed (spec: SA-8).
fn remove_previous(dir: &LibraryDir) -> Result<()> {
    let path = dir.previous_token_cache_file();
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(gone) if gone.kind() == ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(LocalIoError::new(LocalOperation::Removing, path, cause).into()),
    }
}
