use std::sync::Arc;

use coffret_model::Passphrase;
use tracing::info;

use crate::account_dir::AccountDir;
use crate::account_grant::{self, LibraryGrant, Opened};
use crate::account_name::AccountName;
use crate::account_settings::AccountSettings;
use crate::accounts::Accounts;
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::drive;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::reach::Reach;
use crate::referencing_passphrase::ReferencingPassphrase;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Which grant a renewal is for.
///
/// A grant belongs to an account on this device, and renewing it replaces that
/// account's one cache, so every Library that references the account uses the
/// renewed grant from its next run and none is renewed on its own
/// (spec: SA-8). It is named either way a person knows it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizeRequest {
    /// The account a Library references, named by the Library.
    Library {
        /// The Library on this device.
        name: String,
        /// The account to bring the Library onto, for a Library that
        /// references none yet — one whose own previous grant cannot go to
        /// `default`, or one with no grant at all. For a Library that already
        /// references an account it has to be that one: the name is bound into
        /// the Library's envelope and cannot be changed yet (spec: SA-9).
        account: Option<String>,
    },
    /// An account this device holds, by its device-local name.
    Account {
        /// The account's name on this device.
        name: String,
    },
}

/// Runs the authorization flow again for an account this device holds.
///
/// A grant is not forever. While a consent screen is in testing Google expires
/// a refresh token after a week, and a person may revoke one at any time, so
/// renewing a grant is an ordinary thing a device does rather than a repair.
/// Everything else is untouched: the Master Keys, the catalogs, and the
/// mappings are what they were, and only the account's sealed cache is
/// replaced.
///
/// The Passphrase is asked for once the Library or the account has been found,
/// and before anything else: the account's cache is sealed under a key kept
/// only in the envelopes of the Libraries that reference it (spec: SA-9), so
/// there is nothing to write without unlocking one — the Library named, or for
/// an account, whichever of its Libraries the Passphrase opens. A Library or an
/// account that is not here, and a Library not on Drive, are refusals that need
/// no key. A Passphrase that opens nothing ends the call with the cache exactly
/// as it was (spec: DK-2, DK-5), and so does a flow the person abandons: the
/// gateway writes the new cache only once the grant is in hand, and writes it
/// through a rename.
///
/// A Library a build before accounts put here has its own grant promoted into
/// an account first, as opening it would (spec: SA-8), and that account's grant
/// is what is renewed.
pub async fn authorize<P, F>(
    request: AuthorizeRequest,
    enter_passphrase: P,
    open_url: F,
) -> Result<()>
where
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    authorize_through(&Reach::this_device(), request, enter_passphrase, open_url).await
}

/// [`authorize`], reaching Drive through `reach`.
pub(crate) async fn authorize_through<P, F>(
    reach: &Reach,
    request: AuthorizeRequest,
    enter_passphrase: P,
    open_url: F,
) -> Result<()>
where
    P: FnOnce() -> Result<Passphrase> + Send,
    F: FnOnce(&str) + Send,
{
    let (account, credentials) = match request {
        AuthorizeRequest::Library { name, account } => {
            library_account(reach, &name, account.as_deref(), enter_passphrase).await?
        }
        AuthorizeRequest::Account { name } => named_account(&name, enter_passphrase)?,
    };

    let transport = reach.drive_transport()?;
    let cache = drive::token_cache(&account.account, account.key);
    drive::consent(&transport, credentials, cache, open_url).await?;

    // Ordinary progress, and the one event that says a device's access to an
    // account was renewed rather than granted. Which account is the person's
    // and no event's (spec: EL-1); no part of the grant is recorded.
    info!(
        operation = "authorize",
        "renewed this device's grant on an account"
    );
    Ok(())
}

/// The account a Library references, opened through the Library, and the
/// client it renews through.
async fn library_account<P>(
    reach: &Reach,
    name: &str,
    account: Option<&str>,
    enter_passphrase: P,
) -> Result<(Opened, google_drive_store::ClientCredentials)>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    let dir = LibraryDir::resolve(name)?;
    let mut settings = DeviceSettings::read(&dir)?;
    let ProviderSettings::Drive {
        client_id,
        client_secret,
        ..
    } = &settings.provider
    else {
        return Err(Error::NotADriveLibrary {
            name: dir.name().to_owned(),
        });
    };
    let credentials = drive::credentials(client_id, client_secret.as_deref());
    let requested = account.map(AccountName::parse).transpose()?;
    let accounts = Accounts::open()?;

    let passphrase = enter_passphrase()?;
    let unlocked = StoredMasterKeyFile::unlock(&dir, &passphrase)?;
    let grant = account_grant::of_library(
        reach,
        &dir,
        &mut settings,
        &unlocked.master_key,
        &passphrase,
        &accounts,
        requested.as_ref(),
    )
    .await?;
    let opened = match grant {
        LibraryGrant::Opened(opened) => opened,
        LibraryGrant::Unbound => account_grant::bind(
            &dir,
            &mut settings,
            &unlocked.master_key,
            &passphrase,
            &accounts,
            requested,
        )?,
    };

    // An account whose directory went — its settings with it — is given them
    // back, from the Library that names it, before its grant is.
    if !opened.account.is_present() {
        AccountSettings::new(credentials.client_id(), credentials.client_secret())
            .write(&opened.account)?;
    }
    Ok((opened, credentials))
}

/// The account called `name`, opened through whichever Library referencing it
/// the Passphrase unlocks, and the client its grant was issued to.
fn named_account<P>(
    name: &str,
    enter_passphrase: P,
) -> Result<(Opened, google_drive_store::ClientCredentials)>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    let name = AccountName::parse(name)?;
    let accounts = Accounts::open()?;
    let account: AccountDir = accounts
        .get(&name)
        .cloned()
        .ok_or_else(|| Error::NoSuchAccount {
            account: name.as_str().to_owned(),
        })?;
    let settings = AccountSettings::read(&account)?.ok_or_else(|| Error::NoSuchAccount {
        account: name.as_str().to_owned(),
    })?;

    let passphrase = enter_passphrase()?;
    let key = accounts.key_for(&name, &passphrase, &ReferencingPassphrase::unasked())?;
    let credentials = drive::credentials(&settings.client_id, settings.client_secret.as_deref());
    Ok((
        Opened {
            account,
            key: Arc::new(key),
        },
        credentials,
    ))
}

#[cfg(test)]
mod tests;
