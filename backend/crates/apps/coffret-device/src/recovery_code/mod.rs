use coffret_format::RecoveryCode;
use coffret_model::Passphrase;

use crate::device_settings::DeviceSettings;
use crate::error::Result;
use crate::library_dir::LibraryDir;
use crate::library_on_device::LibraryOnDevice;
use crate::library_ref::LibraryRef;
use crate::stored_master_key_file::StoredMasterKeyFile;

/// Writes out the Recovery Code of the Library `library` names.
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
/// a different one (spec: DK-5). It is asked for only once the Library has been
/// found on this device and its stored form found in its directory: a Library
/// that is not here needs no key to be refused, so a mistyped name costs nobody
/// a Passphrase.
///
/// The code comes back with the Library it is the code of: its ID, and its
/// name where this device knows one.
pub fn recovery_code<P>(
    library: &LibraryRef,
    enter_passphrase: P,
) -> Result<(RecoveryCode, LibraryOnDevice)>
where
    P: FnOnce() -> Result<Passphrase>,
{
    let dir = LibraryDir::find(library)?;
    let settings = DeviceSettings::read(&dir)?;
    let unlocked = StoredMasterKeyFile::unlock_asking(&dir, enter_passphrase)?;
    Ok((
        RecoveryCode::encode(unlocked.master_key, unlocked.epoch),
        dir.on_device(&settings),
    ))
}

#[cfg(test)]
mod tests;
