use coffret_format::StoredMasterKey;
use coffret_model::Passphrase;

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Keeps this device's Master Key of the Library called `name` under a new
/// Passphrase.
///
/// Each device has a Passphrase of its own, and what it protects is that
/// device's stored form of the Master Key and nothing else (spec: DK-6). So a
/// change is the stored form written again: the Master Key and its epoch are
/// the ones the current Passphrase opens, unchanged, and nothing on Storage —
/// no Container, no control object — is read or written. Storage is never
/// opened at all, which is why the change needs no network and no grant that
/// is still alive. Every other device keeps the Passphrase it had.
///
/// The new stored form is made from scratch rather than edited: a salt and a
/// nonce of its own, and this build's initial cost, so the key protecting it
/// is derived afresh from the new Passphrase (spec: KD-6). It replaces the old
/// file in one rename, so the device holds one stored form or the other and
/// never neither.
///
/// The current Passphrase is asked for only once the stored form has been
/// found, as [`recovery_code`](crate::recovery_code) asks: a Library that is
/// not here costs nobody a Passphrase. The new one is asked for only once the
/// current one has opened the stored form, so a wrong current Passphrase is
/// refused before a new one is chosen — and refused by the format crate's own
/// authentication, with nothing written (spec: DK-5).
///
/// A process that already holds the Library open keeps the keys it derived
/// until it locks; the change reaches it at its next unlock, which reads the
/// stored form again.
pub fn change_passphrase<E, C>(name: &str, enter_current: E, choose_new: C) -> Result<()>
where
    E: FnOnce() -> Result<Passphrase>,
    C: FnOnce() -> Result<Passphrase>,
{
    let dir = LibraryDir::resolve(name)?;
    let unlocked = StoredMasterKeyFile::unlock_asking(&dir, enter_current)?;
    let new = choose_new()?;
    let stored = StoredMasterKey::create(&new, &unlocked.master_key, unlocked.epoch)
        .map_err(|cause| Error::KeyMaterial { cause })?;
    StoredMasterKeyFile::write(&dir, &stored)
}

#[cfg(test)]
mod tests;
