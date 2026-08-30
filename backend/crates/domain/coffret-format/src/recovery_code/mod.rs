//! The form a Master Key takes on paper.
//!
//! The encoding is normative in KD-11; this module implements it. A Recovery
//! Code is the Master Key and its epoch in one Bech32m string, which is what a
//! user prints at Library creation and types into the next device they add.
//!
//! Bech32m is here for the transcription, not for the cryptography: its
//! alphabet leaves out the four characters people confuse on paper (`1`, `b`,
//! `i`, `o`), and its checksum catches the substitutions and transpositions a
//! hand copy actually makes, so a mistyped code is refused rather than silently
//! read as a different key.
//!
//! Nothing here is Passphrase-derived and nothing here reaches Storage (KD-8).
//! This module deals in one string; printing it, and reading one a user typed,
//! belong to the layer that talks to a person.

use std::fmt;

use bech32::Hrp;
use coffret_model::{MasterKey, MasterKeyEpoch};

mod encode;
mod parse;

#[cfg(test)]
mod tests;

/// A Master Key and its epoch, as the string a user writes down.
///
/// The value is the pair; the string is how it travels. Both are key material,
/// so `Debug` is redacted and only [`Display`](fmt::Display) — the deliberate
/// act of printing a code for its owner — puts the characters anywhere.
#[derive(Clone)]
pub struct RecoveryCode {
    master_key: MasterKey,
    epoch: MasterKeyEpoch,
    /// The canonical lowercase spelling, which a parsed code is re-derived to:
    /// an uppercase or grouped copy is the same code, and one form of it is
    /// what this type hands on.
    text: String,
}

/// The bytes a Recovery Code carries before the five-bit regrouping.
///
/// The version leads and the epoch follows it, so a later version byte can
/// change everything after itself.
mod offset {
    pub(super) const VERSION: usize = 0;
    pub(super) const EPOCH: std::ops::Range<usize> = 1..9;
    pub(super) const MASTER_KEY: usize = 9;
}

impl RecoveryCode {
    /// The human-readable part every Recovery Code starts with.
    pub const HUMAN_READABLE_PART: &'static str = "coffret";

    /// The version this crate writes and reads.
    pub const VERSION: u8 = 0x01;

    /// Length of the payload the string carries, in bytes.
    pub const PAYLOAD_LEN: usize = 1 + 8 + MasterKey::BYTE_LEN;

    /// How many characters the payload takes once regrouped into five-bit
    /// units: 41 bytes are 328 bits, which fill 66 characters and leave two
    /// padding bits.
    pub const DATA_LEN: usize = 66;

    /// Length of the whole lowercase string: `coffret1`, the data, the checksum.
    pub const TEXT_LEN: usize = Self::HUMAN_READABLE_PART.len() + 1 + Self::DATA_LEN + 6;

    /// How many characters one printed group holds.
    pub const GROUP_LEN: usize = 4;

    /// The Master Key this code carries.
    pub fn master_key(&self) -> &MasterKey {
        &self.master_key
    }

    /// The epoch that key belongs to.
    pub fn epoch(&self) -> MasterKeyEpoch {
        self.epoch
    }

    /// The code as one lowercase string, the form the checksum covers.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The code as it is printed: everything after `coffret1` in groups of
    /// four, the data characters and the checksum alike.
    ///
    /// Grouping is presentation and not part of the form — [`parse`] strips it
    /// along with any other whitespace — so a code printed this way and the
    /// same code typed back as one run of characters are one value (KD-11).
    ///
    /// [`parse`]: Self::parse
    pub fn to_grouped_string(&self) -> String {
        let separator = Self::HUMAN_READABLE_PART.len() + 1;
        let (prefix, data) = self.text.split_at(separator);

        let groups = data.len().div_ceil(Self::GROUP_LEN);
        let mut grouped = String::with_capacity(self.text.len() + groups);
        grouped.push_str(prefix);
        for (index, character) in data.chars().enumerate() {
            if index % Self::GROUP_LEN == 0 {
                grouped.push(' ');
            }
            grouped.push(character);
        }
        grouped
    }
}

/// The human-readable part as the encoder and the reader compare it.
const HRP: Hrp = Hrp::parse_unchecked(RecoveryCode::HUMAN_READABLE_PART);

impl fmt::Display for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for RecoveryCode {
    /// Redacted, like [`MasterKey`]'s: the string is the key, so a derived
    /// formatter would put a Library's Master Key in any log line that ever
    /// prints one of these.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(<redacted>)")
    }
}
