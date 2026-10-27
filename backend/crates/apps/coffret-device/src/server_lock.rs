//! The hold one server has on the Library it serves (spec: LA-8).
//!
//! A server publishes a key into the Library's own directory as it starts
//! ([`ServerKey`](crate::ServerKey)) and admits its callers by it. That file is
//! one Library's, not one process's, so a second server starting over the same
//! Library would write its own key over the first one's — and the first server,
//! still up and still holding the Library open, would go on running as a process
//! that answers 403 to everybody, the explorer's own proxy included. The port is
//! no defence: it is an argument, and two servers on two ports over one Library
//! is exactly the case that breaks.
//!
//! So a server takes this lock before it opens the Library, and a start that
//! finds it held is refused. Nothing of the running server's is touched on the
//! way — it keeps its key, its callers and its hold — because the refusal comes
//! before the first thing that would disturb any of them.
//!
//! The lock is the operating system's rather than a file this crate writes and
//! believes. That is the whole of why a killed server leaves nothing to clean
//! up: `flock` is released however the holder ends, `SIGKILL` included, so what
//! the next start finds is a lock nobody holds and it simply takes it. Nothing
//! has to be deleted by hand, which is what LA-4 already promises about the key
//! file beside it.
//!
//! The same lock is what moving a Library's directory to the one its ID names
//! holds while it moves it — a command as it opens the Library, or a server
//! before it takes its own lock (spec: LA-8) — since a server holds the
//! files in the directory by their paths. The file says which of the two holds
//! it, so that a server starting meanwhile is told the directory is being moved
//! rather than that a server it would look for in vain is serving the Library.
//!
//! A `flock` belongs to an open file description, so two of them from one
//! process conflict exactly as two processes do — which is what lets the rule be
//! stated as a test rather than as a run of the binary.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process;

use coffret_usecase::{LocalIoError, LocalOperation};
use rustix::fs::{flock, FlockOperation};
use rustix::io::Errno;

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::owner_only;

/// One server's hold on one Library, for as long as that server runs.
///
/// Held rather than used: there is nothing to ask it, and the value exists to be
/// kept alive. A server drops it by ending, however it ends.
pub struct ServerLock {
    /// The open file description the lock belongs to.
    ///
    /// Never read, and closed when this is dropped — which is what releases the
    /// lock.
    _file: File,
}

impl ServerLock {
    /// Takes the hold on `dir`, or says who has it.
    ///
    /// The refusal is [`Error::LibraryAlreadyServed`], or
    /// [`Error::LibraryBeingMoved`] where a move of the directory holds the
    /// lock, and nothing else: an operating system that declined to arbitrate
    /// at all is a local failure like any other, and reading it as "somebody
    /// is serving this Library" would name a server that is not there.
    ///
    /// The directory has to be one a Library is in. A caller asks
    /// [`LibraryDir::is_present`] first, so that a Library that is not on this
    /// device is refused by whoever opens it rather than by the file this would
    /// try to create beside nothing.
    pub fn take(dir: &LibraryDir) -> Result<Self> {
        Self::take_as(dir, Holder::Server)
    }

    /// Takes the hold on `dir` for as long as its directory is moved to the
    /// one its ID names, or says who has it.
    ///
    /// The hold a server takes, recorded as a move's, so that a server
    /// starting meanwhile is refused with [`Error::LibraryBeingMoved`] rather
    /// than told a server is serving the Library.
    pub(crate) fn take_to_move(dir: &LibraryDir) -> Result<Self> {
        Self::take_as(dir, Holder::Move)
    }

    /// Takes the hold on `dir` as `holder`, or says who has it.
    fn take_as(dir: &LibraryDir, holder: Holder) -> Result<Self> {
        let path = dir.server_lock_file();
        // Opened as a creation, not as a lock: a directory this cannot write
        // into, or a disk with no room, refused the file, and the line a person
        // reads has to send them to that rather than to the filesystem's lock.
        let mut file = owner_only::open_or_create_file(&path, LocalOperation::Creating)?;

        match flock(&file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => {}
            // The one errno that is an answer rather than a failure: somebody
            // else holds it, which is the state this whole module exists to
            // report.
            Err(Errno::WOULDBLOCK) => {
                let library = dir.which();
                return Err(match holder_of(&mut file) {
                    Some((Holder::Move, by)) => Error::LibraryBeingMoved { library, by },
                    Some((Holder::Server, by)) => Error::LibraryAlreadyServed { library, by },
                    None => Error::LibraryAlreadyServed { library, by: None },
                });
            }
            Err(cause) => {
                return Err(Error::Local(LocalIoError::new(
                    LocalOperation::Locking,
                    path,
                    cause.into(),
                )))
            }
        }

        // After the lock and never before it: whatever a previous server wrote
        // stays readable to whoever is refused until this one has actually won,
        // and two starts racing cannot leave the file naming the one that lost.
        write_holder(&mut file, &path, holder)?;
        Ok(Self { _file: file })
    }
}

/// What holds the lock: a server, or a move of the directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
    /// A server serving the Library, which is stopped to free it.
    Server,
    /// A process moving the directory to the one its ID names — a command
    /// opening the Library, or a server before it takes its own lock — which
    /// frees it by itself once the rename is done.
    Move,
}

/// The word a move writes before its number; a server writes its number alone,
/// as every build before moves did.
const MOVE_WORD: &str = "move";

/// What the holder of the lock wrote down, where it wrote anything: which kind
/// of holder it is, and its number where that reads.
///
/// Nothing here is trusted: it is read for a sentence, and a file holding
/// something else — a holder killed between taking the lock and writing its
/// number, or anything at all — leaves the sentence one clause shorter rather
/// than turning a refusal into a failure. A file naming no move is read as a
/// server's, which is what the lock is for.
fn holder_of(file: &mut File) -> Option<(Holder, Option<u32>)> {
    let mut written = String::new();
    file.read_to_string(&mut written).ok()?;
    let written = written.trim();
    Some(match written.strip_prefix(MOVE_WORD) {
        Some(number) => (Holder::Move, number.trim().parse().ok()),
        None => (Holder::Server, written.parse().ok()),
    })
}

/// Writes what holds the lock and this process's number, so that whoever is
/// refused next is told whether a server is to be stopped, and which, or a
/// move waited for.
fn write_holder(file: &mut File, path: &Path, holder: Holder) -> Result<()> {
    let written = match holder {
        Holder::Server => process::id().to_string(),
        Holder::Move => format!("{MOVE_WORD} {}", process::id()),
    };
    file.set_len(0)
        .map_err(Error::local(LocalOperation::Writing, path))?;
    file.write_all(written.as_bytes())
        .map_err(Error::local(LocalOperation::Writing, path))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::server_key::ServerKey;
    use crate::testing::state_dir;

    /// A Library directory of this run's own, made ready to be written into.
    ///
    /// Every case in this crate resolves against one state directory per test
    /// binary, so a directory among the Libraries is shared with every other
    /// case in it rather than local to this module. The names here say what
    /// they are about *and* that they are this module's.
    fn directory(name: &str) -> LibraryDir {
        state_dir();
        let dir = LibraryDir::beside(name).expect("the state directory is set");
        owner_only::create_dir(dir.path()).expect("the state directory is writable");
        dir
    }

    /// What a second start over `dir` was refused with, or a panic saying what
    /// happened instead.
    fn refusal(dir: &LibraryDir) -> Error {
        match ServerLock::take(dir) {
            Err(error) => error,
            Ok(_) => panic!("a second server over one Library must be refused"),
        }
    }

    // LA-8: one server at a time, and the first one is left exactly as it was:
    // it keeps the key it published, so its callers go on being admitted and its
    // hold on the Library is never even asked about.
    #[test]
    fn a_second_server_for_one_library_is_refused() {
        let dir = directory("server-lock-one-at-a-time");
        let key = ServerKey::publish(&dir).expect("the first server publishes its key");
        let _first = ServerLock::take(&dir).expect("the first server takes the lock");

        let refused = refusal(&dir);
        assert!(
            matches!(
                &refused,
                Error::LibraryAlreadyServed { library, .. } if *library == dir.which()
            ),
            "expected a second server over one Library to be refused, got {refused:?}"
        );

        // And the second start got nowhere near the running server's key: the
        // refusal happens before a Passphrase is asked for, let alone before
        // anything is published.
        assert_eq!(
            fs::read_to_string(key.path()).expect("the key file must still be readable"),
            key.secret(),
        );
    }

    // LA-8, the sentence: the Library, so the person knows which of theirs it is
    // about, and the process, so they can find the server they have to stop.
    #[test]
    fn the_refusal_names_the_library_and_the_process_holding_it() {
        let dir = directory("server-lock-names-the-holder");
        let _first = ServerLock::take(&dir).expect("the first server takes the lock");

        let refused = refusal(&dir);
        assert!(
            matches!(
                &refused,
                Error::LibraryAlreadyServed { by: Some(by), .. } if *by == process::id()
            ),
            "expected the refusal to name the process holding the lock, got {refused:?}"
        );

        let said = refused.to_string();
        assert!(said.contains(dir.name()), "{said}");
        assert!(said.contains(&process::id().to_string()), "{said}");
    }

    // LA-8, the other half of the sentence. The key is not what this is about
    // and neither is the file it is in; nor is the file the lock is on, which is
    // this crate's own arrangement and the one thing deleting would not help —
    // the lock goes when the process does (spec: LA-4).
    #[test]
    fn the_refusal_names_neither_the_key_nor_the_file_it_is_in() {
        let dir = directory("server-lock-says-nothing-of-the-key");
        let key = ServerKey::publish(&dir).expect("the first server publishes its key");
        let _first = ServerLock::take(&dir).expect("the first server takes the lock");

        let said = refusal(&dir).to_string();
        for named in [
            key.secret(),
            &key.path().display().to_string(),
            &dir.server_key_file().display().to_string(),
            &dir.server_lock_file().display().to_string(),
            "server-key",
            "server.lock",
        ] {
            assert!(!said.contains(named), "{said} names {named}");
        }
    }

    // LA-8 with LA-4: the lock is the operating system's, so a server that was
    // killed released it without knowing it had. What the next start finds is a
    // file nobody holds, and it takes it — with nothing having been deleted by
    // hand, and the file still where it was.
    #[test]
    fn a_lock_a_stopped_server_left_behind_is_taken_by_the_next_one() {
        let dir = directory("server-lock-left-behind");

        let stopped = ServerLock::take(&dir).expect("the first server takes the lock");
        drop(stopped);

        assert!(
            dir.server_lock_file().is_file(),
            "the file stays where it is"
        );
        let _next = ServerLock::take(&dir).expect("the next server takes what nobody holds");
    }

    // LA-8. The rule is one Library's and not this device's: a device may serve
    // as many Libraries as it has, and each one's server holds its own
    // directory's lock.
    #[test]
    fn two_libraries_are_served_at_once() {
        let first = directory("server-lock-first-library");
        let second = directory("server-lock-second-library");

        let _one = ServerLock::take(&first).expect("the first Library is served");
        let _other = ServerLock::take(&second).expect("the second Library is served too");
    }

    // LA-8: a server starting while a directory move holds the lock is told
    // the directory is being moved, by which process, and that it can start
    // again — not that a server is serving the Library.
    #[test]
    fn a_lock_a_move_holds_is_refused_as_a_move() {
        let dir = directory("server-lock-held-by-a-move");
        let moving = ServerLock::take_to_move(&dir).expect("the move takes the lock");

        let refused = refusal(&dir);
        assert!(
            matches!(
                &refused,
                Error::LibraryBeingMoved { library, by: Some(by) }
                    if *library == dir.which() && *by == process::id()
            ),
            "expected a refusal saying the directory is being moved, got {refused:?}"
        );
        let said = refused.to_string();
        assert!(said.contains("is being moved"), "{said}");
        assert!(said.contains("run this again"), "{said}");
        assert!(!said.contains("served"), "{said}");

        // And a move that meets a server is told a server holds it.
        drop(moving);
        let _served = ServerLock::take(&dir).expect("the server takes the freed lock");
        assert!(matches!(
            ServerLock::take_to_move(&dir),
            Err(Error::LibraryAlreadyServed { by: Some(_), .. })
        ));
    }

    // LA-8. Owner-only like everything else in this directory, for a reason of
    // its own: nothing in this file is secret, and another account able to take
    // the lock on it is another account able to keep this device's own server
    // from ever starting.
    #[cfg(unix)]
    #[test]
    fn the_lock_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = directory("server-lock-owner-only");
        let _held = ServerLock::take(&dir).expect("the lock is taken");

        let mode = fs::metadata(dir.server_lock_file())
            .expect("the file must be there")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, owner_only::OWNER_ONLY_FILE);
    }
}
