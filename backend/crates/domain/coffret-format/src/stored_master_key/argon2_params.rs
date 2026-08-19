use argon2::{Algorithm, Argon2, Params, Version};

use crate::aead::KEY_LEN;
use crate::error::{Error, Result};

/// The Argon2id cost the Passphrase is stretched at on one device.
///
/// The parameters are device-local policy rather than a format constant: they
/// are recorded in the stored form that used them, so raising them later
/// re-derives the protection key and rewrites only that device's stored Master
/// Key — no Storage Object changes at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl Argon2Params {
    /// The values a device starts with.
    ///
    /// Taken from the OWASP Password Storage Cheat Sheet's Argon2id band —
    /// m=19456 KiB (19 MiB), t=2, p=1, one of the several
    /// (memory, iterations) pairs it lists as equivalent. A device may raise
    /// them at any time under KD-6; the stored form records what it used.
    pub const INITIAL: Self = Self {
        memory_kib: 19_456,
        iterations: 2,
        parallelism: 1,
    };

    /// Takes a cost, whatever Argon2id will accept.
    pub const fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Self {
        Self {
            memory_kib,
            iterations,
            parallelism,
        }
    }

    /// Memory cost in KiB.
    pub const fn memory_kib(self) -> u32 {
        self.memory_kib
    }

    /// Number of passes over that memory.
    pub const fn iterations(self) -> u32 {
        self.iterations
    }

    /// How many lanes the passes are spread across.
    pub const fn parallelism(self) -> u32 {
        self.parallelism
    }

    /// Stretches a Passphrase into the key that protects a stored Master Key.
    pub(super) fn derive(self, passphrase: &[u8], salt: &[u8]) -> Result<[u8; KEY_LEN]> {
        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_LEN),
        )
        .map_err(|error| Error::InvalidArgon2Params {
            detail: error.to_string(),
        })?;

        let mut key = [0u8; KEY_LEN];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(passphrase, salt, &mut key)
            .map_err(|error| Error::PassphraseDerivationFailed {
                detail: error.to_string(),
            })?;
        Ok(key)
    }
}

/// A cost cheap enough to run in a test, and unlike the initial values.
#[cfg(test)]
pub(super) const CHEAP: Argon2Params = Argon2Params::new(8, 1, 1);

#[cfg(test)]
mod tests {
    use super::*;

    // KD-6: the initial values come from the OWASP-recommended band current at
    // release. Pinning them here makes a change to them a deliberate edit.
    #[test]
    fn initial_values_are_the_recommended_ones() {
        assert_eq!(Argon2Params::INITIAL.memory_kib(), 19_456);
        assert_eq!(Argon2Params::INITIAL.iterations(), 2);
        assert_eq!(Argon2Params::INITIAL.parallelism(), 1);
    }

    // KD-5: the Passphrase is stretched with Argon2id, so the same Passphrase
    // and salt give the same key and a different salt gives a different one.
    #[test]
    fn derivation_depends_on_the_passphrase_and_the_salt() {
        let key = CHEAP
            .derive(b"passphrase", b"salt-sixteen-byt")
            .expect("the parameters are valid");
        assert_eq!(
            CHEAP
                .derive(b"passphrase", b"salt-sixteen-byt")
                .expect("the parameters are valid"),
            key
        );
        assert_ne!(
            CHEAP
                .derive(b"passphrase", b"salt-sixteen-oth")
                .expect("the parameters are valid"),
            key
        );
        assert_ne!(
            CHEAP
                .derive(b"other", b"salt-sixteen-byt")
                .expect("the parameters are valid"),
            key
        );
    }

    // KD-6: the parameters drive the derivation, so a different cost over the
    // same Passphrase and salt is a different key.
    #[test]
    fn derivation_depends_on_the_parameters() {
        let key = CHEAP
            .derive(b"passphrase", b"salt-sixteen-byt")
            .expect("the parameters are valid");
        assert_ne!(
            Argon2Params::new(8, 2, 1)
                .derive(b"passphrase", b"salt-sixteen-byt")
                .expect("the parameters are valid"),
            key
        );
    }

    #[test]
    fn parameters_argon2id_refuses_are_reported_as_such() {
        assert!(matches!(
            Argon2Params::new(0, 1, 1).derive(b"passphrase", b"salt-sixteen-byt"),
            Err(Error::InvalidArgon2Params { .. })
        ));
    }
}
