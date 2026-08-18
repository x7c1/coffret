//! The one place this crate draws randomness from.
//!
//! Every identifier, key, salt, and random nonce coffret writes comes from the
//! operating system's CSPRNG through here, so there is a single answer to where
//! the entropy came from and a single place a failure to get it is reported.

use crate::error::{Error, Result};

/// Draws `N` bytes from the operating system's CSPRNG.
pub(crate) fn draw<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|error| Error::EntropyUnavailable {
        detail: error.to_string(),
    })?;
    Ok(bytes)
}
