//! The root of the key hierarchy, and the register of everything under it that
//! holds secret bytes.
//!
//! # The secret-bearing inventory
//!
//! DK-7 promises that after a lock no readable copy of the Master Key remains
//! in the process, and says how that is honored: *key material lives in a type
//! that overwrites itself when dropped and is never copied into a buffer
//! outside that type.* That promise is only checkable against a list, so the
//! list lives here, beside the type the hierarchy hangs from. A new
//! key-bearing type joins it; a type that leaves the unlock path leaves it.
//!
//! Walking the path from what a person types to the key that opens one
//! Container:
//!
//! | type | crate | holds | wiped by |
//! | --- | --- | --- | --- |
//! | [`Passphrase`](crate::Passphrase) | `coffret-model` | the bytes a person typed | its own `Drop` |
//! | protection key | `coffret-format` | Argon2id output over the Passphrase (KD-5) | the `Zeroizing` buffer `Argon2Params::derive` hands back |
//! | stored-form plaintext | `coffret-format` | Master Key ‖ epoch, in and out of the sealed form (KD-7) | `Zeroizing`, inside `StoredMasterKey::create`/`unlock` |
//! | [`MasterKey`] | `coffret-model` | the 256 bits everything else derives from (KD-1) | its own `Drop` |
//! | `UnlockedMasterKey` | `coffret-format` | a `MasterKey` and its epoch | its `MasterKey` field |
//! | `RecoveryCode` | `coffret-format` | a `MasterKey` and the Bech32m text carrying the same bytes (KD-11) | its own `Drop` |
//! | typed code text | `coffret-format` | a Recovery Code as a person wrote it down, before it is a value (KD-11) | `Zeroizing`, inside `recovery_code::parse` |
//! | `PurposeKey` | `coffret-format` | one HKDF output over the Master Key (KD-3) | its own `Drop` |
//! | `ControlKeys` | `coffret-usecase` | the four control-object purpose keys | its `PurposeKey` fields |
//! | `LibraryKeys` | `coffret-usecase` | `ControlKeys` and the container-wrap key | its `PurposeKey` fields |
//! | envelope plaintext | `coffret-format` | a Container Key on its way in or out of a Key Envelope (FM-14) | `Zeroizing`, inside `key_envelope` |
//! | [`ContainerKey`](crate::ContainerKey) | `coffret-model` | the 256 bits that encrypt one Container (KD-2) | its own `Drop` |
//!
//! Two rules keep the list honest, and both are pinned by the assertions in
//! `zeroization.rs`:
//!
//! - every type on it wipes its bytes when it is dropped, either through a
//!   `Drop` of its own or through a field that has one, and says so by
//!   implementing [`zeroize::ZeroizeOnDrop`];
//! - none of them is `Clone`. A derived `Clone` is a copy nobody audits, so a
//!   caller that needs one value in two places borrows it, moves it, or shares
//!   it through an `Arc` — where the copy count stays one and the wipe still
//!   happens, once, at the end.
//!
//! What this does not promise: the operating system may page a buffer to disk
//! before it is wiped, a debugger attached to the process reads whatever it
//! likes, and a `MasterKey` moved from one place to another may leave the bytes
//! of the old location behind — Rust gives no way to intercept a move. DK-7
//! names those same limits — *moves, freed allocations, swap, and core
//! dumps* — as what puts the rule past what a test can observe, and asks for
//! it to be *honored by construction* instead. The inventory is that
//! construction, and it is the whole of what this file claims.

use std::fmt;

use zeroize::{Zeroize, ZeroizeOnDrop};

/// The 256-bit key every purpose key in a Library is derived from.
///
/// It is drawn from a CSPRNG and never from the Passphrase or any other
/// user-chosen input, so the strength of the ciphertext on Storage never
/// depends on passphrase quality. Each Master Key epoch draws its own.
///
/// `Debug` is redacted so key material cannot reach a log line through a
/// derived formatter, and the type deliberately implements neither `Display`
/// nor `PartialEq` — an equality operator would have to be constant-time, and
/// nothing in the domain needs to compare two keys.
///
/// It is neither `Copy` nor `Clone`, and it overwrites its bytes when it is
/// dropped: one key has one owner, and the process holds no readable copy of it
/// past the point that owner ends (spec: DK-7).
pub struct MasterKey([u8; Self::BYTE_LEN]);

impl MasterKey {
    /// Length of a Master Key in bytes.
    pub const BYTE_LEN: usize = 32;

    /// Takes 32 raw bytes.
    ///
    /// The caller's array is left as it was — an array of bytes is `Copy`, and
    /// nothing here can reach back into it — so whoever produced those bytes
    /// wipes them: the generator hands over the only copy, and the decryption
    /// paths that build one read out of a `Zeroizing` buffer.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 32 bytes.
    ///
    /// A borrow and never a copy: everything that uses a Master Key — HKDF,
    /// the stored form's plaintext, a Recovery Code's payload — reads it in
    /// place, so nothing needs the bytes handed over by value.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(<redacted>)")
    }
}

impl Drop for MasterKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl ZeroizeOnDrop for MasterKey {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_does_not_leak_key_material() {
        let key = MasterKey::from_bytes([0xab; MasterKey::BYTE_LEN]);
        assert_eq!(format!("{key:?}"), "MasterKey(<redacted>)");
    }

    // DK-7: the wipe itself is what `Drop` runs, and reading a buffer back
    // after its owner is gone needs `unsafe`, which this crate forbids. So the
    // check is on the operation rather than on the freed memory: the same call
    // the drop makes, over the same field, leaves nothing readable behind.
    #[test]
    fn the_drop_time_wipe_overwrites_the_key() {
        let mut key = MasterKey::from_bytes([0xab; MasterKey::BYTE_LEN]);
        key.0.zeroize();
        assert_eq!(key.as_bytes(), &[0u8; MasterKey::BYTE_LEN]);
    }
}
