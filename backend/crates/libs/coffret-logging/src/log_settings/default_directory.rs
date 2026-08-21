use std::env;
use std::path::PathBuf;

use crate::error::{Error, Result};

/// The default log directory, from the state directory the platform names.
///
/// `$XDG_STATE_HOME` when it is set, and `$HOME/.local/state` — the value the
/// specification defines that variable to default to — when it is not.
pub fn default_directory() -> Result<PathBuf> {
    let state = match env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        Some(state) => PathBuf::from(state),
        None => {
            let home = env::var_os("HOME").filter(|value| !value.is_empty());
            PathBuf::from(home.ok_or(Error::NoStateDirectory)?).join(".local/state")
        }
    };
    Ok(state.join("coffret/logs"))
}
