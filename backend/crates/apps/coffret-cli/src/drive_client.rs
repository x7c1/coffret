//! Which OAuth client the Drive flows authorize as.
//!
//! `init` and `join` both take the client the same way, which is why the
//! reading is here rather than in either of them: the id is typed as
//! `--client-id`, and the secret is read out of `COFFRET_DRIVE_CLIENT_SECRET`.
//! The two arrive differently because they are different things.
//!
//! The id decides which application a Library is created as, and nothing in
//! the output or the settings afterwards says where it came from — so it is
//! typed every time rather than picked up from whatever environment happened
//! to be loaded, which would let an `init` run in the wrong directory put an
//! everyday Library under a test project without a word. There is no built-in
//! id to default to either: registering one is the account owner's to do, and
//! a shared one would put every user of coffret in the same consent screen
//! quota.
//!
//! The secret is not a secret a person holds: `init` stores it in the
//! Library's settings and every later token refresh reads it back from there.
//! It is configuration, and configuration is what the environment is for — a
//! flag would only put it in the shell history and the process table on the
//! way in.

use std::env::VarError;

use anyhow::bail;
use coffret_device::Error as DeviceError;

/// Where the client secret comes from, for a client registered with one.
///
/// A desktop client registered with a secret cannot exchange its authorization
/// code without it. There is no flag for it; see the module doc for why.
pub const CLIENT_SECRET: &str = "COFFRET_DRIVE_CLIENT_SECRET";

/// The secret for the client `--client-id` names, as the environment carries
/// it.
///
/// A secret variable that is not set means a client registered without one,
/// which is a shape a desktop client is allowed to have. A variable set to an
/// empty value is refused instead of read as the same thing: an empty secret
/// is never what was meant, and letting it through would fail later at the
/// token exchange, as a refusal about the grant rather than about the
/// environment.
pub fn client_secret() -> anyhow::Result<Option<String>> {
    match std::env::var(CLIENT_SECRET) {
        Ok(secret) if secret.is_empty() => bail!(
            "{CLIENT_SECRET} is set to an empty value; set it to the secret the client was \
             registered with, or unset it where the client was registered without one"
        ),
        Ok(secret) => Ok(Some(secret)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => bail!("{CLIENT_SECRET} is not valid Unicode"),
    }
}

/// The refusal a person reads, with the variable named where the run
/// authorized without a secret.
///
/// The one thing the layers below cannot say. A client registered with a secret
/// is refused at the code exchange in the same words as a code that expired,
/// and the gateway that knows which of the two it was looking at does not know
/// where a secret would have come from — that is this shell's own vocabulary,
/// and this is the place that reads the variable.
///
/// Said afterwards and never before: a client registered without a secret is a
/// shape a desktop client is allowed to have, so an unset variable is no reason
/// to stop anybody. It is only once the exchange has been refused that the
/// variable is worth mentioning at all.
///
/// For `init` and `join` alone, which are the two commands that read the
/// variable. `authorize` takes a person through the same consent screen and can
/// be refused at the same exchange, but the secret it sends is the one the
/// Library's settings hold — set there by the `init` that created it — so
/// setting the variable is not the way through and naming it would send a
/// person somewhere that changes nothing.
pub fn explaining(error: DeviceError) -> anyhow::Error {
    if error.is_exchange_without_client_secret() {
        return anyhow::Error::new(error).context(format!(
            "nothing was sent for a client secret: where the client --client-id names was \
             registered with one, set {CLIENT_SECRET} to it and run this again"
        ));
    }
    anyhow::Error::new(error)
}
