use argon2::{Algorithm, Argon2, Params, Version};
use coffret_model::Passphrase;
use zeroize::Zeroizing;

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

    /// The most a stored form may ask for, each parameter inclusive
    /// (spec: KD-9).
    ///
    /// A ceiling on what a reader accepts, not a cost anything derives at: a
    /// form within it unlocks under the cost it records. The recorded cost is
    /// authenticated only once the derivation it asks for has run, so without
    /// a ceiling a form edited to an enormous cost would exhaust memory or
    /// never finish before the Passphrase is judged. A device strengthening
    /// its cost stays within it (spec: KD-6).
    pub const CEILING: Self = Self {
        memory_kib: 262_144,
        iterations: 8,
        parallelism: 4,
    };

    /// Takes a cost, whatever Argon2id will accept.
    ///
    /// Nothing is checked here: a cost above [`CEILING`](Self::CEILING) is
    /// refused where it would be written or read, before any derivation.
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

    /// This cost, if no parameter of it exceeds [`CEILING`](Self::CEILING)
    /// (spec: KD-9).
    ///
    /// The refusal names the first parameter over its ceiling, and nothing is
    /// retried at a lower cost: a form that asks for more than a reader
    /// accepts is refused, never opened under a cost it does not record.
    pub(super) fn within_ceiling(self) -> Result<Self> {
        let checks = [
            (
                "memory cost in KiB",
                self.memory_kib,
                Self::CEILING.memory_kib,
            ),
            ("iterations", self.iterations, Self::CEILING.iterations),
            ("parallelism", self.parallelism, Self::CEILING.parallelism),
        ];
        match checks
            .into_iter()
            .find(|(_, value, ceiling)| value > ceiling)
        {
            Some((parameter, value, ceiling)) => Err(Error::Argon2CostAboveCeiling {
                parameter,
                value,
                ceiling,
            }),
            None => Ok(self),
        }
    }

    /// Stretches a Passphrase into the key that protects a stored Master Key.
    ///
    /// The output is a `Zeroizing` buffer rather than a bare array: it is a key
    /// in every sense that matters — whoever holds it opens this device's
    /// stored form — and it is short-lived, so the one thing it must not do is
    /// outlive its use in freed memory (spec: DK-7). Callers hold it in place
    /// and hand `Cipher::new` a borrow of it.
    pub(super) fn derive(
        self,
        passphrase: &Passphrase,
        salt: &[u8],
    ) -> Result<Zeroizing<[u8; KEY_LEN]>> {
        let params = Params::new(
            self.memory_kib,
            self.iterations,
            self.parallelism,
            Some(KEY_LEN),
        )
        .map_err(|cause| Error::InvalidArgon2Params { cause })?;

        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(passphrase.as_bytes(), salt, &mut *key)
            .map_err(|cause| Error::PassphraseDerivationFailed { cause })?;
        Ok(key)
    }
}

// The writer must never produce a form the reader refuses (spec: KD-6), so the
// build does not compile if the initial cost exceeds the ceiling.
const _: () = assert!(
    Argon2Params::INITIAL.memory_kib <= Argon2Params::CEILING.memory_kib
        && Argon2Params::INITIAL.iterations <= Argon2Params::CEILING.iterations
        && Argon2Params::INITIAL.parallelism <= Argon2Params::CEILING.parallelism
);

/// A cost cheap enough to run in a test, and unlike the initial values.
#[cfg(test)]
pub(super) const CHEAP: Argon2Params = Argon2Params::new(8, 1, 1);

#[cfg(test)]
mod tests {
    use super::*;

    /// The Passphrase these cases stretch, as the type the derivation takes.
    fn passphrase(bytes: &[u8]) -> Passphrase {
        Passphrase::from_bytes(bytes.to_vec())
    }

    // KD-6: the initial values come from the OWASP-recommended band current at
    // release. Pinning them here makes a change to them a deliberate edit.
    #[test]
    fn initial_values_are_the_recommended_ones() {
        assert_eq!(Argon2Params::INITIAL.memory_kib(), 19_456);
        assert_eq!(Argon2Params::INITIAL.iterations(), 2);
        assert_eq!(Argon2Params::INITIAL.parallelism(), 1);
    }

    // KD-6: the initial values lie within the ceilings KD-9 sets on what a
    // reader accepts, so every form a device writes is one it can read back.
    #[test]
    fn initial_values_are_within_the_ceiling() {
        assert!(matches!(
            Argon2Params::INITIAL.within_ceiling(),
            Ok(params) if params == Argon2Params::INITIAL
        ));
    }

    // KD-9: the ceilings are pinned here, so a change to them is a deliberate
    // edit to the specification as well.
    #[test]
    fn the_ceiling_is_the_specified_one() {
        assert_eq!(Argon2Params::CEILING.memory_kib(), 262_144);
        assert_eq!(Argon2Params::CEILING.iterations(), 8);
        assert_eq!(Argon2Params::CEILING.parallelism(), 4);
    }

    // KD-9: each ceiling is inclusive, and one past it is refused, naming the
    // parameter and the ceiling.
    #[test]
    fn a_cost_above_a_ceiling_is_refused_and_named() {
        let ceiling = Argon2Params::CEILING;
        assert!(matches!(ceiling.within_ceiling(), Ok(params) if params == ceiling));

        let cases = [
            (
                Argon2Params::new(262_145, 1, 1),
                "memory cost in KiB",
                262_145,
                262_144,
            ),
            (Argon2Params::new(8, 9, 1), "iterations", 9, 8),
            (Argon2Params::new(40, 1, 5), "parallelism", 5, 4),
            (
                Argon2Params::new(4_194_304, u32::MAX, 1),
                "memory cost in KiB",
                4_194_304,
                262_144,
            ),
            (Argon2Params::new(8, u32::MAX, 1), "iterations", u32::MAX, 8),
        ];
        for (params, parameter, value, ceiling) in cases {
            let result = params.within_ceiling();
            assert!(
                matches!(
                    result,
                    Err(Error::Argon2CostAboveCeiling { parameter: p, value: v, ceiling: c })
                        if p == parameter && v == value && c == ceiling
                ),
                "{params:?} gave {result:?}"
            );
        }
    }

    // KD-5: the Passphrase is stretched with Argon2id, so the same Passphrase
    // and salt give the same key and a different salt gives a different one.
    #[test]
    fn derivation_depends_on_the_passphrase_and_the_salt() {
        let key = CHEAP
            .derive(&passphrase(b"passphrase"), b"salt-sixteen-byt")
            .expect("the parameters are valid");
        assert_eq!(
            CHEAP
                .derive(&passphrase(b"passphrase"), b"salt-sixteen-byt")
                .expect("the parameters are valid"),
            key
        );
        assert_ne!(
            CHEAP
                .derive(&passphrase(b"passphrase"), b"salt-sixteen-oth")
                .expect("the parameters are valid"),
            key
        );
        assert_ne!(
            CHEAP
                .derive(&passphrase(b"other"), b"salt-sixteen-byt")
                .expect("the parameters are valid"),
            key
        );
    }

    // KD-6: the parameters drive the derivation, so a different cost over the
    // same Passphrase and salt is a different key.
    #[test]
    fn derivation_depends_on_the_parameters() {
        let key = CHEAP
            .derive(&passphrase(b"passphrase"), b"salt-sixteen-byt")
            .expect("the parameters are valid");
        assert_ne!(
            Argon2Params::new(8, 2, 1)
                .derive(&passphrase(b"passphrase"), b"salt-sixteen-byt")
                .expect("the parameters are valid"),
            key
        );
    }

    // And the refusal names which parameter it was about, since the cause
    // travels as the value Argon2id reported rather than as a sentence.
    #[test]
    fn parameters_argon2id_refuses_are_reported_as_such() {
        assert!(matches!(
            Argon2Params::new(0, 1, 1).derive(&passphrase(b"passphrase"), b"salt-sixteen-byt"),
            Err(Error::InvalidArgon2Params {
                cause: argon2::Error::MemoryTooLittle
            })
        ));
    }
}
