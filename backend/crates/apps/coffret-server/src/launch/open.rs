use std::sync::Arc;

use anyhow::Context;
use coffret_device::{open_library, LibraryDir, Passphrase, ServerKey, ServerLock};
use tokio::net::TcpListener;

use super::{Launch, Serving};
use crate::{catch_up_at_startup, Admission, ServerState};

impl Launch {
    /// Opens the Library and binds a socket for it, without serving anything
    /// yet.
    ///
    /// `enter_passphrase` is asked only once every refusal that needs no key
    /// has passed, exactly as [`open_library`] asks it: a Library that is not on
    /// this device, or one another server is already serving, costs nobody a
    /// prompt.
    ///
    /// Every step but the catch-up is fatal where it fails, and the error is
    /// the sentence a person is shown: the whole chain, `{:#}`, is what says
    /// what failed and why.
    pub async fn open<P>(self, enter_passphrase: P) -> anyhow::Result<Serving>
    where
        P: FnOnce() -> coffret_device::Result<Passphrase> + Send,
    {
        let directory = LibraryDir::resolve(&self.library)?;

        // Before the Passphrase, deliberately, and so before anything else
        // here: one server at a time serves a Library (spec: LA-8), and being
        // told that after typing a Passphrase would be being asked for it to no
        // purpose. The server already running is left exactly as it is — its
        // key, its callers and its hold on the Library are none of this
        // process's business.
        //
        // Only for a Library that is on this device, so that one which is not is
        // refused by `open_library` in its own words rather than by a file this
        // would try to create beside nothing. Held by the `Serving` this
        // returns, for as long as it serves: the lock is the open file
        // description's, and dropping it would hand the Library to the next
        // server while this one is still serving it.
        let hold = directory
            .is_present()
            .then(|| ServerLock::take(&directory))
            .transpose()?;

        // Before the socket, deliberately. Every refusal opening a Library owes
        // — it is not on this device, the Passphrase does not open it, the
        // grant has run out — is one a person acts on, and a server that had
        // already bound a port would state it once per request instead of once.
        let library = open_library(&self.library, enter_passphrase).await?;
        let state = Arc::new(ServerState::new(self.library.clone(), library));

        // Before the socket as well, and for a different reason than the unlock
        // above: not because the refusal has to come before anything is bound,
        // but because the first window this server answers ought to show the
        // Library rather than whatever this device knew when it last looked
        // (spec: CK-9) — a device that has just joined knowing nothing at all.
        // It is on a deadline of its own, so a Storage that answers neither yes
        // nor no delays the socket rather than withholding it.
        catch_up_at_startup(&state).await;

        // Before the socket for the reason the unlock is: a key that could not
        // be drawn or could not be written is a server nothing legitimate could
        // ask anything of, and one that had already bound a port would say so
        // once per request instead of once. It replaces whatever a previous run
        // left, so the file a caller reads is always this server's. What it
        // replaces is never a running server's: the lock above would have
        // refused this process long before here if one were up (spec: LA-8).
        let key = ServerKey::publish(&directory)?;

        // Loopback and nothing else (spec: LA-1): these routes carry the
        // Library's plaintext, and an interface anybody else is on would be that
        // plaintext offered to whoever else is on the network. Who is answered
        // *on* this device is the key's business rather than the address's. See
        // the crate documentation.
        let address = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&address)
            .await
            .with_context(|| format!("nothing could listen at {address}"))?;

        // What was bound rather than what was asked for. Port `0` is how a
        // caller asks the operating system for a free one, and the number it
        // chose is then the only one anything can reach the Library at.
        let bound = listener
            .local_addr()
            .context("the address the server is listening at could not be read")?;

        Ok(Serving {
            address: bound,
            key_file: key.path().to_path_buf(),
            idle: self.idle_interval(),
            admission: Arc::new(Admission::new(bound.to_string(), key.secret())),
            state,
            listener,
            _hold: hold,
        })
    }
}
