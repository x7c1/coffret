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
