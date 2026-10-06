//! The grant an open Drive Library reaches its Storage through, held so that a
//! process that stays up can renew it without the Passphrase.

use std::fmt;
use std::sync::Arc;

use coffret_model::AccountCacheKey;
use google_drive_store::ClientCredentials;
use tracing::info;

use crate::account_dir::AccountDir;
use crate::account_grant::Opened;
use crate::account_settings::AccountSettings;
use crate::drive;
use crate::error::Result;

/// The account an open Drive Library references, and the client its grant was
/// issued to.
///
/// What [`authorize`](crate::authorize) spends the Passphrase to reach: the
/// account's directory, the account-cache key the Library's envelope yields
/// (spec: KD-12), and the OAuth client the Library's settings name. An open
/// Library already holds the key — the store it reaches Drive through reads
/// the account's cache with it on every mint (spec: SA-6) — so keeping it
/// here as well adds no reach and no lifetime: it is the same key, shared, and
/// it goes when the Library does (spec: DK-7).
///
/// Nothing about it is readable from outside this crate. What a caller does
/// with one is [`renew`](Self::renew) it.
#[derive(Clone)]
pub struct DriveGrant {
    account: AccountDir,
    key: Arc<AccountCacheKey>,
    credentials: ClientCredentials,
}

impl DriveGrant {
    /// The grant of the account `opened` names, through `credentials`.
    pub(crate) fn of(opened: &Opened, credentials: ClientCredentials) -> Self {
        Self {
            account: opened.account.clone(),
            key: Arc::clone(&opened.key),
            credentials,
        }
    }

    /// Runs the authorization flow again for this account, handing `open_url`
    /// the consent page, and keeps the grant it yields in the account's cache.
    ///
    /// The flow [`authorize`](crate::authorize) runs, from the point at which
    /// it has the account open: the same client (spec: SA-1, SA-2), the same
    /// verification before anything is cached (spec: SA-4), and the same one
    /// cache replaced (spec: SA-6). No Passphrase is asked for, because nothing
    /// here needs one that the open Library does not already hold. A flow the
    /// person declines or leaves unanswered leaves the cache exactly as it was.
    ///
    /// The cache is the one every reader of the account mints from, so a store
    /// already open over it — the one this Library reaches Drive through —
    /// mints its next access token from the renewed grant, with no reopening.
    pub async fn renew<F>(&self, open_url: F) -> Result<()>
    where
        F: FnOnce(&str) + Send,
    {
        // An account whose directory went — its settings with it — is given them
        // back before its grant is, as `authorize` does.
        if !self.account.is_present() {
            AccountSettings::new(
                self.credentials.client_id(),
                self.credentials.client_secret(),
            )
            .write(&self.account)?;
        }
        let transport = drive::transport()?;
        let cache = drive::token_cache(&self.account, Arc::clone(&self.key));
        drive::consent(&transport, self.credentials.clone(), cache, open_url).await?;

        // The event `authorize` writes, under the operation that asked. Which
        // account is the person's and no event's (spec: EL-1).
        info!(
            operation = "reconnect",
            "renewed this device's grant on an account from a running Library"
        );
        Ok(())
    }
}

impl fmt::Debug for DriveGrant {
    /// Says it is one and nothing else: the account's directory is a path on
    /// this device, and the key is not a thing to render at all.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DriveGrant { .. }")
    }
}
