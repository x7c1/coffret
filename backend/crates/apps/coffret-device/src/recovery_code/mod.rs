use coffret_format::RecoveryCode;
use coffret_model::Passphrase;

use crate::error::Result;
use crate::library_dir::LibraryDir;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Writes out the Recovery Code of the Library called `name`.
///
/// The code is not stored anywhere: it is the Master Key and its epoch in the
/// form a person writes down (spec: KD-11), and it is written out again here
/// from the stored form this device's Passphrase opens. Which is exactly why
/// this call exists — a person who lost the printout has not lost the code, so
/// long as one device still holds the Library and that device's Passphrase is
/// known.
///
/// A Passphrase that does not open the stored form yields the format crate's
/// own refusal and no bytes at all, so a wrong one produces no code rather than
/// a different one (spec: DK-5). It is asked for only once the name has been
/// found to be one path component and the stored form has been found where a
/// Library of that name would keep it: a Library that is not here needs no key
/// to be refused, so a mistyped name costs nobody a Passphrase.
pub fn recovery_code<P>(name: &str, enter_passphrase: P) -> Result<RecoveryCode>
where
    P: FnOnce() -> Result<Passphrase>,
{
    let dir = LibraryDir::resolve(name)?;
    let unlocked = StoredMasterKeyFile::unlock_asking(&dir, enter_passphrase)?;
    Ok(RecoveryCode::encode(unlocked.master_key, unlocked.epoch))
}

#[cfg(test)]
mod tests;
