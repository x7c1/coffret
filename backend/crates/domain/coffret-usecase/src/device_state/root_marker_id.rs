use std::error;
use std::fmt;

use coffret_model::lowercase_hex;

/// The identity one mapped root folder carries, which a device records when it
/// records the mapping (spec: EP-13).
///
/// Distinct from [`RootIdentity`](super::RootIdentity) in what it answers.
/// That one is the filesystem the root stood on when a scan last looked, read
/// off the platform and used to tell an unmounted mount point from a folder the
/// user emptied (spec: EP-12). This one is not read off anything: it is drawn
/// here, written into the root as a marker file, and kept beside the mapping,
/// so that before a device places a file it can ask whether the folder in front
/// of it is the folder that was registered.
///
/// Eight random bytes, spelled as sixteen lowercase hexadecimal characters
/// exactly the way a Library ID (FM-18) and a Container ID (FM-3) are spelled,
/// so the text written into the marker, the text read back out of it, and the
/// text compared against are one form. What that text is a marker file's whole
/// content, and what makes one malformed, is
/// [`root_marker`](crate::root_marker)'s.
///
/// What it is not: a secret, an authenticator, or a claim about a volume. A
/// faithful copy of a registered folder carries a faithful copy of the marker
/// and is indistinguishable from it, and nothing here resists somebody writing
/// whichever sixteen characters they like. It certifies that the folder is the
/// one that was registered against an ordinary mistake — a disk mounted
/// elsewhere, a root recorded and then moved — and nothing more (spec: EP-13).
///
/// It never leaves the device, like the mappings it belongs to (spec: CK-7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootMarkerId([u8; Self::BYTE_LEN]);

impl RootMarkerId {
    /// Length of a root's identity in bytes.
    pub const BYTE_LEN: usize = 8;

    /// Length of a root's identity in hex characters.
    pub const HEX_LEN: usize = Self::BYTE_LEN * 2;

    /// Takes 8 raw bytes.
    pub const fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw 8 bytes.
    pub const fn as_bytes(&self) -> &[u8; Self::BYTE_LEN] {
        &self.0
    }

    /// Draws a fresh identity from the operating system's CSPRNG.
    ///
    /// The same source every other identifier coffret writes comes from, and
    /// reached the same way a Library ID reaches it. Random rather than derived
    /// from anything about the folder — its path, its filesystem, the time —
    /// because two roots that must be told apart may agree on every one of
    /// those, and because a root recorded again under a new path has to keep
    /// the identity it already carries (spec: EP-13).
    pub fn generate() -> coffret_format::Result<Self> {
        coffret_format::draw_random_bytes().map(Self)
    }

    /// Reads the identity back from its sixteen-character spelling.
    pub fn parse(hex: &str) -> Result<Self, MalformedRootMarkerId> {
        lowercase_hex::decode(hex)
            .map(Self)
            .map_err(|cause| MalformedRootMarkerId { cause })
    }

    /// The 16-lowercase-hex-character spelling.
    pub fn to_hex(&self) -> String {
        lowercase_hex::encode(&self.0)
    }
}

impl fmt::Display for RootMarkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// A text that is not the spelling of a root's identity (spec: EP-13).
///
/// Deliberately no `PartialEq`, for the reason every other error type here has
/// none: what a caller does with a refusal is report it, and comparing two of
/// them is not something the type should invite. `Clone` it does have, because
/// a refusal about a mapped root travels on into a finding a run hands whoever
/// asked for it (spec: EP-13), and a finding read off a borrowed outcome has to
/// copy the reason rather than take it.
#[derive(Debug, Clone)]
pub struct MalformedRootMarkerId {
    cause: coffret_model::Error,
}

impl MalformedRootMarkerId {
    /// What the hex reading refused it for: the length, or the character.
    pub fn cause(&self) -> &coffret_model::Error {
        &self.cause
    }
}

impl fmt::Display for MalformedRootMarkerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "not the {} lowercase hexadecimal characters a root's identity is spelled as",
            RootMarkerId::HEX_LEN
        )
    }
}

impl error::Error for MalformedRootMarkerId {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.cause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RootMarkerId {
        RootMarkerId::from_bytes([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77])
    }

    // EP-13: the identifier is eight random bytes, so two roots registered
    // moments apart are told apart.
    #[test]
    fn drawing_twice_gives_two_identities() {
        let first = RootMarkerId::generate().expect("the OS CSPRNG is available");
        let second = RootMarkerId::generate().expect("the OS CSPRNG is available");

        assert_ne!(first, second);
        assert_eq!(first.as_bytes().len(), RootMarkerId::BYTE_LEN);
    }

    // The one text form: what is written into the marker is what parses back,
    // and it is spelled the way a Library ID is (spec: EP-13, FM-18).
    #[test]
    fn the_hex_spelling_round_trips() {
        let id = sample();

        assert_eq!(id.to_hex(), "0011223344556677");
        assert_eq!(id.to_string(), id.to_hex());
        assert_eq!(
            RootMarkerId::parse(&id.to_hex()).expect("an identity's own spelling parses back"),
            id
        );
    }

    // Uppercase is a hex digit nowhere in coffret, so a marker spelled with one
    // is refused rather than taken as a second spelling of the same identity.
    #[test]
    fn a_spelling_that_is_not_sixteen_lowercase_hex_characters_is_refused() {
        for hex in [
            "",
            "001122334455667",
            "00112233445566778",
            "zzzzzzzzzzzzzzzz",
        ] {
            let parsed = RootMarkerId::parse(hex);
            assert!(
                parsed.is_err(),
                "expected {hex:?} to be refused, got {parsed:?}"
            );
        }

        let uppercase = RootMarkerId::parse("00112233445566AA");
        assert!(
            matches!(
                uppercase.as_ref().err().map(MalformedRootMarkerId::cause),
                Some(coffret_model::Error::InvalidHexDigit { found: 'A' })
            ),
            "expected an uppercase digit to be named as what refused it, got {uppercase:?}"
        );
    }
}
