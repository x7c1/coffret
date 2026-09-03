//! The key a running server admits its callers by.
//!
//! A server on this device binds a loopback port and answers with the Library's
//! plaintext. Being able to reach that port is not the same as being the person
//! who owns the Library: the owner's own browser runs other people's pages, and
//! any of them may send a request at a loopback port. So a server draws a key as
//! it starts, writes it here, and answers nobody who cannot show it.
//!
//! The file is what makes that a boundary rather than a formality. It is
//! owner-only, as everything else this device keeps for the Library is, so a
//! caller that can read it is a process of the account that owns the Library —
//! and a page in a browser is not one, because a page cannot read a local file.
//! Nothing is on the URL, so the key reaches no `Referer` header, no shell
//! history and no access log on the way.
//!
//! A key per run, not a key per Library. Nothing carries from one process to the
//! next, so a key that leaked is spent when the server it belonged to stops, and
//! a file left behind by a server that was killed opens nothing.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::owner_only;

/// How many bytes of the operating system's CSPRNG the key is drawn from.
///
/// Thirty-two, which is what everything else coffret draws a secret at. Nobody
/// ever types this one — a caller reads it from the file — so there is nothing
/// to trade length against.
const KEY_BYTES: usize = 32;

/// One server's key, drawn as it started.
///
/// Held rather than read back: what the server compares a request against is
/// the value it drew, so a file edited under a running process changes what its
/// callers must read and never what it will accept.
pub struct ServerKey {
    secret: String,
    path: PathBuf,
}

impl ServerKey {
    /// Draws a key for this run and puts it where a caller on this device can
    /// read it.
    ///
    /// Whatever a previous run left is replaced, through the same rename every
    /// other file here is written by: a caller reading while this happens sees
    /// one key or the other and never half of one.
    ///
    /// An entropy source that refuses stops the server, unlike the batch names
    /// this crate also draws random bytes for. A key anything could guess is not
    /// a weaker boundary than this one — it is no boundary at all.
    pub fn publish(dir: &LibraryDir) -> Result<Self> {
        let mut bytes = [0_u8; KEY_BYTES];
        getrandom::fill(&mut bytes).map_err(|cause| Error::ServerKeyNotDrawn {
            detail: cause.to_string(),
        })?;
        let secret: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();

        let path = dir.server_key_file();
        owner_only::write_file("writing the server's key", &path, secret.as_bytes())?;
        Ok(Self { secret, path })
    }

    /// The key itself, which every request to the routes has to carry.
    pub fn secret(&self) -> &str {
        &self.secret
    }

    /// The file a caller on this device reads it out of.
    ///
    /// The path and never the key: this is what a server may name on a terminal
    /// so that whoever is running it can point their own tooling at the file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::testing::state_dir;

    /// A Library directory of this run's own, made ready to be written into.
    ///
    /// Every case in this crate resolves against one state directory per test
    /// binary, so a Library name is shared with every other case in it rather
    /// than local to this module. The names here say what they are about *and*
    /// that they are this module's.
    fn directory(name: &str) -> LibraryDir {
        state_dir();
        let dir = LibraryDir::resolve(name).expect("the name is one path component");
        owner_only::create_dir("making a Library directory", dir.path())
            .expect("the state directory is writable");
        dir
    }

    // The key is what a caller shows, so it has to be in the file a caller
    // reads — byte for byte, with nothing around it to be trimmed off wrongly.
    #[test]
    fn the_file_holds_the_key_the_server_will_accept() {
        let dir = directory("server-key-published");
        let key = ServerKey::publish(&dir).expect("a key is drawn and written");

        assert_eq!(
            fs::read_to_string(key.path()).expect("the file must be readable"),
            key.secret(),
        );
        assert_eq!(key.path(), dir.server_key_file());
        assert_eq!(key.secret().len(), KEY_BYTES * 2);
    }

    // The file's mode is the whole of the boundary: whoever can read it can ask
    // the running server for the Library's plaintext.
    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let key =
            ServerKey::publish(&directory("server-key-owner-only")).expect("a key is written");

        let mode = fs::metadata(key.path())
            .expect("the file must be there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, owner_only::OWNER_ONLY_FILE);
    }

    // One key per run. A server that starts over a directory another server
    // already wrote into replaces what is there, so the key a caller reads is
    // always the one the running server will accept.
    #[test]
    fn a_second_run_draws_a_key_of_its_own() {
        let dir = directory("server-key-redrawn");
        let first = ServerKey::publish(&dir).expect("the first run draws one");
        let second = ServerKey::publish(&dir).expect("the second run draws another");

        assert_ne!(first.secret(), second.secret());
        assert_eq!(
            fs::read_to_string(dir.server_key_file()).expect("the file must be readable"),
            second.secret(),
        );
    }
}
