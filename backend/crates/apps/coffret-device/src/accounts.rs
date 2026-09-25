//! The directory a device keeps its accounts in, and the choice of which one a
//! Library references.

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use coffret_model::{AccountCacheKey, Passphrase, Redacted};
use coffret_usecase::{LocalIoError, LocalOperation};
use tracing::{debug, info, warn};

use crate::account_dir::AccountDir;
use crate::account_envelope::AccountEnvelope;
use crate::account_name::AccountName;
use crate::account_settings::AccountSettings;
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{Error, Result};
use crate::library_dir::{accounts_root, libraries_root, LibraryDir, STAGING_SUFFIX};
use crate::referencing_passphrase::ReferencingPassphrase;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// What this is recorded as.
const OPERATION: &str = "open_accounts";

/// The accounts this device holds, and which of its Libraries reference each.
///
/// Opened rather than read: opening it is the moment a cache no Library
/// references any longer is discarded (spec: SA-8). The device knows which
/// those are without opening any envelope, because every Library's settings
/// name the account it references — so an account no Library's settings name is
/// one whose last envelope went with its Library, and its cache can no longer
/// be opened by anything.
pub(crate) struct Accounts {
    /// Every account this device holds, by name.
    held: BTreeMap<AccountName, AccountDir>,
    /// Every whole Library that names an account, by the name it gives.
    references: BTreeMap<String, Vec<LibraryDir>>,
}

/// Which account a Library being put on this device is to reference.
#[derive(Debug)]
pub(crate) enum Choice {
    /// One the device already holds.
    Held(AccountDir),
    /// One the device does not hold yet, and gains with this Library's consent.
    New(AccountName),
}

impl Accounts {
    /// Reads which accounts are held and which Libraries reference them, and
    /// discards every account no Library references (spec: SA-8).
    ///
    /// Nothing is discarded where a Library's settings could not be read: an
    /// account that Library might name would otherwise go with a grant it still
    /// needs, and keeping a cache a little longer costs nothing. What is being
    /// built is left alone for the same reason — a Library in its staging
    /// directory names nothing yet, and the account it will reference is still
    /// in a staging directory of its own until the Library is in place.
    pub(crate) fn open() -> Result<Self> {
        let (references, unsure) = references()?;

        let mut held = BTreeMap::new();
        let mut kept_unsure = 0usize;
        for name in entries(&accounts_root()?)? {
            // Not a name this build gives an account: somebody else's, or a
            // staging directory, built now or left by an interrupted attempt.
            let Ok(account) = AccountName::parse(&name) else {
                continue;
            };
            let dir = AccountDir::resolve(&account)?;
            if !references.contains_key(account.as_str()) {
                if unsure {
                    kept_unsure += 1;
                } else {
                    discard(&dir)?;
                    continue;
                }
            }
            if dir.is_present() {
                held.insert(account, dir);
            }
        }
        if kept_unsure > 0 {
            warn!(
                operation = OPERATION,
                kept = kept_unsure,
                "kept account caches no Library was seen to reference, because the settings of a \
                 Library could not be read"
            );
        }

        Ok(Self { held, references })
    }

    /// The account called `name`, where this device holds one.
    pub(crate) fn get(&self, name: &AccountName) -> Option<&AccountDir> {
        self.held.get(name)
    }

    /// Every account this device holds.
    pub(crate) fn held(&self) -> impl Iterator<Item = &AccountDir> {
        self.held.values()
    }

    /// How many accounts this device holds.
    pub(crate) fn count(&self) -> usize {
        self.held.len()
    }

    /// Which account a Library being put here references, given the name the
    /// person asked for, if any (spec: SA-8).
    ///
    /// A name is optional while the device holds one account — that one is
    /// used, or an account called [`AccountName::DEFAULT`] is made where there
    /// is none — and required once it holds more, since from then on the name
    /// is the only thing that says which grant is meant.
    pub(crate) fn choose(&self, requested: Option<AccountName>) -> Result<Choice> {
        if let Some(name) = requested {
            return Ok(match self.get(&name) {
                Some(dir) => Choice::Held(dir.clone()),
                None => Choice::New(name),
            });
        }
        let mut held = self.held.values();
        match (held.next(), held.next()) {
            (None, _) => Ok(Choice::New(AccountName::default_name())),
            (Some(only), None) => Ok(Choice::Held(only.clone())),
            (Some(_), Some(_)) => Err(Error::AccountNameRequired { held: self.count() }),
        }
    }

    /// The account-cache key of `account`, through a Library that references it
    /// and opens with `passphrase`.
    ///
    /// The key is kept nowhere but in those Libraries' envelopes, each under
    /// that Library's own Master Key (spec: SA-9), so a device reaches it only
    /// by unlocking one of them. `None` where the Passphrase opens none of
    /// them, or where every one it opens holds an envelope that does not: the
    /// account is there and this Passphrase is not a way into it.
    pub(crate) fn key_through(
        &self,
        account: &AccountName,
        passphrase: &Passphrase,
    ) -> Option<AccountCacheKey> {
        self.opened_through(account, passphrase).ok()
    }

    /// The same, or why not: `Err(None)` where the Passphrase unlocks none of
    /// the Libraries, and otherwise the first refusal of an envelope in one it
    /// unlocked.
    fn opened_through(
        &self,
        account: &AccountName,
        passphrase: &Passphrase,
    ) -> std::result::Result<AccountCacheKey, Option<Error>> {
        let libraries = self
            .references
            .get(account.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut unread = None;
        for dir in libraries {
            match Self::key_in(dir, account, passphrase) {
                None => {}
                Some(Ok(key)) => return Ok(key),
                // Another Library referencing the account may still open it.
                Some(Err(refused)) => {
                    warn!(
                        operation = OPERATION,
                        reason = %refused.redacted(),
                        "a Library referencing the account opened, and its envelope did not"
                    );
                    unread.get_or_insert(refused);
                }
            }
        }
        Err(unread)
    }

    /// The first Library that references `account`, by its device-local name:
    /// the one a person is asked the Passphrase of, and the one a refusal names.
    pub(crate) fn first_referencing(&self, account: &AccountName) -> Option<&LibraryDir> {
        self.references
            .get(account.as_str())
            .and_then(|libraries| libraries.first())
    }

    /// The account-cache key of `account`: through `passphrase` where it opens
    /// a Library that references the account, and otherwise through the
    /// Passphrase `referencing` is asked for one of them (spec: SA-9).
    ///
    /// One question at most, and about one Library, named: where there is
    /// nobody to ask, or what was given does not open it, the refusal names the
    /// Library whose Passphrase would — never a consent that would keep a
    /// second cache for the account (spec: SA-8). A Library that did open and
    /// whose envelope did not is refused as that instead: the Passphrase was
    /// right, and naming another Library's would send the person after the
    /// wrong thing.
    pub(crate) fn key_for(
        &self,
        account: &AccountName,
        passphrase: &Passphrase,
        referencing: &ReferencingPassphrase,
    ) -> Result<AccountCacheKey> {
        let unread = match self.opened_through(account, passphrase) {
            Ok(key) => return Ok(key),
            Err(unread) => unread,
        };
        let Some(library) = self.first_referencing(account) else {
            // An account no Library references is discarded as the accounts
            // are opened, so this is one another run took away meanwhile.
            return Err(Error::NoSuchAccount {
                account: account.as_str().to_owned(),
            });
        };
        let not_opened = || Error::AccountNotOpened {
            account: account.as_str().to_owned(),
            library: library.name().to_owned(),
        };
        let Some(given) = referencing.ask(library.name()) else {
            return Err(unread.unwrap_or_else(not_opened));
        };
        let given = given?;
        Self::key_in(library, account, &given).unwrap_or_else(|| Err(not_opened()))
    }

    /// The account-cache key of `account` out of the envelope of `library`,
    /// where `passphrase` unlocks it; `None` where it does not, and the
    /// envelope's refusal where it unlocks and the envelope does not open.
    fn key_in(
        library: &LibraryDir,
        account: &AccountName,
        passphrase: &Passphrase,
    ) -> Option<Result<AccountCacheKey>> {
        let unlocked = match StoredMasterKeyFile::unlock(library, passphrase) {
            Ok(unlocked) => unlocked,
            Err(refused) => {
                debug!(
                    operation = OPERATION,
                    reason = %refused.redacted(),
                    "a Library referencing the account did not open with this Passphrase"
                );
                return None;
            }
        };
        Some(AccountEnvelope::open(
            library,
            &unlocked.master_key,
            account,
        ))
    }

    /// Refuses a Library whose client is not the one `account` was consented
    /// to (spec: SA-8).
    pub(crate) fn require_client(
        account: &AccountDir,
        library: &str,
        library_client: &str,
    ) -> Result<()> {
        let Some(settings) = AccountSettings::read(account)? else {
            return Ok(());
        };
        if settings.client_id == library_client {
            return Ok(());
        }
        Err(Error::ClientMismatch(Box::new(
            crate::error::ClientMismatch {
                library: library.to_owned(),
                library_client: library_client.to_owned(),
                account: account.name().as_str().to_owned(),
                account_client: settings.client_id,
            },
        )))
    }
}

/// A new account, built beside where it goes until the Library that references
/// it is in place.
///
/// Built apart for the reason a Library is: until a Library's settings name it,
/// the account is one no Library references, and a device opening its accounts
/// in the meantime would discard it (spec: SA-8). So it takes its real name only
/// once the Library naming it has taken its own.
///
/// Dropped without being published, it takes what it built with it.
pub(crate) struct NewAccount {
    dir: AccountDir,
    staging: AccountDir,
    published: bool,
}

impl NewAccount {
    /// Starts building the account called `name`, recording the client its
    /// grant is to be issued to.
    pub(crate) fn begin(name: &AccountName, settings: &AccountSettings) -> Result<Self> {
        let dir = AccountDir::resolve(name)?;
        let staging = dir.staging();
        remove_dir(staging.path())?;
        settings.write(&staging)?;
        Ok(Self {
            dir,
            staging,
            published: false,
        })
    }

    /// The directory the account's cache is written into until it is in place.
    pub(crate) fn staged(&self) -> &AccountDir {
        &self.staging
    }

    /// Moves the account to the name it is known by.
    pub(crate) fn publish(mut self) -> Result<AccountDir> {
        fs::rename(self.staging.path(), self.dir.path())
            .map_err(Error::local(LocalOperation::Renaming, self.dir.path()))?;
        self.published = true;
        Ok(self.dir.clone())
    }
}

impl Drop for NewAccount {
    /// Takes out what was built, for an attempt that did not finish.
    fn drop(&mut self) {
        if self.published {
            return;
        }
        if let Err(refused) = remove_dir(self.staging.path()) {
            warn!(
                operation = OPERATION,
                reason = %refused.redacted(),
                "could not remove an account an interrupted attempt left"
            );
        }
    }
}

/// Which account every whole Library on this device names, and whether the
/// settings of any of them could not be read.
fn references() -> Result<(BTreeMap<String, Vec<LibraryDir>>, bool)> {
    let mut references: BTreeMap<String, Vec<LibraryDir>> = BTreeMap::new();
    let mut unsure = false;
    for name in entries(&libraries_root()?)? {
        if name.ends_with(STAGING_SUFFIX) {
            continue;
        }
        let Ok(dir) = LibraryDir::resolve(&name) else {
            continue;
        };
        if !dir.is_present() {
            continue;
        }
        match DeviceSettings::read(&dir) {
            Ok(DeviceSettings {
                provider:
                    ProviderSettings::Drive {
                        account: Some(account),
                        ..
                    },
                ..
            }) => references.entry(account).or_default().push(dir),
            Ok(_) => {}
            Err(refused) => {
                unsure = true;
                warn!(
                    operation = OPERATION,
                    reason = %refused.redacted(),
                    "the settings of a Library could not be read"
                );
            }
        }
    }
    Ok((references, unsure))
}

/// The names in the directory at `path`, or none where there is no directory.
fn entries(path: &Path) -> Result<Vec<String>> {
    let listing = match fs::read_dir(path) {
        Ok(listing) => listing,
        Err(cause) if cause.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(cause) => return Err(LocalIoError::new(LocalOperation::Reading, path, cause).into()),
    };
    let mut names = Vec::new();
    for entry in listing {
        let entry = entry.map_err(Error::local(LocalOperation::Reading, path))?;
        // A name that is not Unicode is not one this build gave anything.
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Discards an account no Library references (spec: SA-8).
fn discard(dir: &AccountDir) -> Result<()> {
    remove_dir(dir.path())?;
    // Which account is the person's to know and no event's (spec: EL-1): what
    // is kept is that one went.
    info!(
        operation = OPERATION,
        "discarded the grant of an account no Library on this device references"
    );
    Ok(())
}

/// Removes a directory this device made for its own purposes, where absence is
/// the outcome being sought (spec: OC-8).
fn remove_dir(path: &Path) -> std::result::Result<(), LocalIoError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(gone) if gone.kind() == ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(LocalIoError::new(LocalOperation::Removing, path, cause)),
    }
}
