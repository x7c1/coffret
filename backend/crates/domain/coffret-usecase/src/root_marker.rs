//! The device's own management area inside a mapped root, and the marker file
//! that gives the root an identity.
//!
//! Two rules meet here. A mapped root carries an identity of its own — a marker
//! file holding a random identifier, kept beside the mapping as what that
//! mapping expects to find, so that a device can ask whether the folder in front
//! of it is the folder that was registered before it places anything into it
//! (spec: EP-13). And the folder that marker stands in is the device's own,
//! reserved by name at any depth under a mapped root: a scan never enters it and
//! never reports anything under it as a file to back up, and nothing is ever
//! placed at a path carrying the name (spec: EP-14).
//!
//! The two rules share this module because they share a name. The reservation
//! is what makes the marker's own file invisible to the scan that walks the
//! folder holding it, so a second spelling of [`MANAGEMENT_AREA`] anywhere would
//! be a name the scan does not know to step over — the same trap the reserved
//! scratch prefix beside it avoids the same way (see [`scratch`](crate::scratch)).
//!
//! Nothing here does any I/O. What is written, adopted, or refused when a
//! mapping is recorded is the device layer's; what a marker's bytes *are* is
//! this module's, so one answer serves the writing and the reading alike.

use std::error;
use std::fmt;
use std::str;

use coffret_model::EntryPath;

use crate::device_state::{MalformedRootMarkerId, RootMarkerId};

/// The name reserved for the device's own management area, as a path component
/// at any depth under a mapped root (spec: EP-14).
///
/// A scan decides from the name alone, exactly as it does for the scratch
/// prefix (spec: EP-11). The cost is the one that reservation states — anything
/// of the user's own under a folder of this name is not backed up.
pub const MANAGEMENT_AREA: &str = ".coffret";

/// The marker file inside the management area, whose content is the root's
/// identity (spec: EP-13).
pub const MARKER_FILE: &str = "root";

/// The most of a marker file a device reads.
///
/// A marker holds sixteen characters and at most a newline, so anything past
/// this is not a marker somebody wrote — it is a file that happens to stand at
/// the name. Reading a bounded amount is what keeps a root pointed at a huge or
/// endless file from being a way to make a device read it (spec: EP-13).
pub const MAX_LEN: usize = 64;

/// Whether one local name is the device's own management area rather than
/// content to back up (spec: EP-14).
pub fn is_management_area(name: &str) -> bool {
    name == MANAGEMENT_AREA
}

/// Whether an Entry Path carries the reserved name at any depth (spec: EP-14).
///
/// The placement side of the same reservation the scan reads by name. A scan
/// asks [`is_management_area`] of each local name as it walks; a placement has
/// no walk to ask it during — the question is settled before a single component
/// is descended — so it asks it of the Entry Path's own components instead.
///
/// Name-only and no I/O, which is what keeps it here beside the reservation it
/// reads rather than in whichever flow places a file. A path carrying the
/// component at *any* depth is refused, because the name is reserved at any
/// depth: the management area of a mapped root standing inside another mapped
/// root is such a component under the outer one, so one question answers the
/// overlapping case too.
pub fn carries_management_area(path: &EntryPath) -> bool {
    path.as_str().split('/').any(is_management_area)
}

/// The bytes a marker file holds for `id`: the sixteen characters and one
/// newline.
///
/// The newline is written and not merely tolerated, so that the file reads as a
/// line of text in whatever a person opens it with. What [`parse`] accepts is
/// wider than what this produces by exactly that newline, which is the whole of
/// the tolerance (spec: EP-13).
pub fn spell(id: &RootMarkerId) -> Vec<u8> {
    let mut bytes = id.to_hex().into_bytes();
    bytes.push(b'\n');
    bytes
}

/// The identity a marker file's bytes name, or why they name none.
///
/// Exactly the sixteen characters, with at most one trailing newline and
/// nothing else: no leading space, no second newline, no carriage return, and
/// nothing at all past [`MAX_LEN`]. A marker is written by coffret and read by
/// coffret, so there is no spelling to be generous towards — and being generous
/// would mean a file that is *nearly* a marker deciding what a device places
/// into somebody's folder (spec: EP-13).
pub fn parse(bytes: &[u8]) -> Result<RootMarkerId, MalformedMarker> {
    if bytes.len() > MAX_LEN {
        return Err(MalformedMarker::TooLong { read: bytes.len() });
    }
    let spelling = match bytes.strip_suffix(b"\n") {
        Some(without_newline) => without_newline,
        None => bytes,
    };
    let spelling = str::from_utf8(spelling).map_err(|_| MalformedMarker::NotText)?;
    RootMarkerId::parse(spelling).map_err(|cause| MalformedMarker::NotAnIdentity { cause })
}

/// Why a marker file's content is no identity (spec: EP-13).
///
/// Deliberately no `PartialEq`: a caller reports which of these it was and does
/// not compare two of them. `Clone` it does have, because a placement's refusal
/// carries one on into a finding a run hands whoever asked for it (spec: EP-13),
/// and a finding read off a borrowed outcome copies the reason rather than
/// taking it.
#[derive(Debug, Clone)]
pub enum MalformedMarker {
    /// More bytes than a marker may hold.
    ///
    /// A reader stops one byte past [`MAX_LEN`], which is the least it can read
    /// and still see that the content runs on. So this says the content is
    /// longer than a marker may be rather than how long the file really is.
    TooLong {
        /// How many bytes were read before the length settled it: one past
        /// [`MAX_LEN`] where the reader stopped there.
        read: usize,
    },
    /// The bytes are not text at all.
    NotText,
    /// The text is not the spelling of an identity.
    NotAnIdentity {
        /// What the reading refused it for.
        cause: MalformedRootMarkerId,
    },
}

impl MalformedMarker {
    /// What made the content no identity, in a word fit for a diagnostic event
    /// (spec: EL-1).
    ///
    /// The content itself never reaches one — it is a file out of somebody's
    /// folder, whatever it turned out to hold — so what an event records is
    /// which of the three ways it failed. Said here rather than at each of the
    /// two callers that log one, so that both spell the same defect the same
    /// way.
    pub fn defect(&self) -> &'static str {
        match self {
            Self::TooLong { .. } => "past the cap",
            Self::NotText => "not text",
            Self::NotAnIdentity { .. } => "not an identity",
        }
    }
}

impl fmt::Display for MalformedMarker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { read } => write!(
                f,
                "a marker holds at most {MAX_LEN} bytes and this one holds at least {read}"
            ),
            Self::NotText => f.write_str("a marker's content is text and this is not"),
            Self::NotAnIdentity { cause } => write!(f, "{cause}"),
        }
    }
}

impl error::Error for MalformedMarker {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::TooLong { .. } | Self::NotText => None,
            Self::NotAnIdentity { cause } => Some(cause),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path;

    fn sample() -> RootMarkerId {
        RootMarkerId::from_bytes([0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77])
    }

    // EP-13: what a device writes is what a device reads back, and the reading
    // is the writing's inverse and nothing wider.
    #[test]
    fn a_written_marker_parses_back() {
        let id = sample();
        let written = spell(&id);

        assert_eq!(written, b"0011223344556677\n");
        assert_eq!(
            parse(&written).expect("a marker coffret wrote must parse"),
            id
        );
    }

    // The newline is optional on the way in, because that is the one shape a
    // person editing the file by hand is likely to leave it in either way.
    #[test]
    fn the_trailing_newline_is_optional() {
        assert_eq!(
            parse(b"0011223344556677").expect("a marker without the newline must parse"),
            sample()
        );
    }

    // Everything else, and each for its own reason: a marker is coffret's own
    // file, so a file that is only nearly one names no identity at all.
    #[test]
    fn anything_else_is_malformed() {
        for content in [
            &b""[..],
            b"\n",
            // Two newlines, a carriage return, and leading space: near misses
            // that a generous reading would take for the marker they resemble.
            b"0011223344556677\n\n",
            b"0011223344556677\r\n",
            b" 0011223344556677\n",
            // Uppercase is a hex digit nowhere in coffret (spec: FM-3, FM-18).
            b"0011223344556677AA",
            b"001122334455667",
            b"not sixteen hex!",
        ] {
            let parsed = parse(content);
            assert!(
                matches!(parsed, Err(MalformedMarker::NotAnIdentity { .. })),
                "expected {content:?} to name no identity, got {parsed:?}"
            );
        }

        let not_text = parse(&[0xff, 0xfe, b'\n']);
        assert!(
            matches!(not_text, Err(MalformedMarker::NotText)),
            "expected bytes that are no text to be refused as such, got {not_text:?}"
        );
    }

    // The cap is the reading's own bound, so a file standing at the marker's
    // name is refused on its length before anything is made of its content.
    #[test]
    fn content_past_the_cap_is_refused_on_its_length() {
        let long = vec![b'0'; MAX_LEN + 1];
        let parsed = parse(&long);
        assert!(
            matches!(parsed, Err(MalformedMarker::TooLong { read }) if read == MAX_LEN + 1),
            "expected content past the cap to be refused for its length, got {parsed:?}"
        );

        let at_the_cap = parse(&[b'0'; MAX_LEN]);
        assert!(
            matches!(at_the_cap, Err(MalformedMarker::NotAnIdentity { .. })),
            "at the cap it is the spelling that refuses it, got {at_the_cap:?}"
        );
    }

    // EP-14: the reserved name is decided from the name alone, and it is one
    // name rather than a prefix — a folder whose name merely begins with it is
    // the user's own.
    #[test]
    fn only_the_reserved_name_is_the_management_area() {
        assert!(is_management_area(MANAGEMENT_AREA));
        assert!(!is_management_area(".coffret-fetch-abc.part"));
        assert!(!is_management_area(".coffretish"));
        assert!(!is_management_area("coffret"));
    }

    // EP-14: the reservation is at any depth, so the placement side asks it of
    // every component of the Entry Path rather than of the first one — a path
    // whose `.coffret` is three folders down is the same reserved name.
    #[test]
    fn a_reserved_component_is_found_at_any_depth() {
        for reserved in [
            ".coffret/root",
            "albums/.coffret/root",
            "albums/2026/.coffret",
        ] {
            assert!(
                carries_management_area(&entry_path(reserved)),
                "{reserved} carries the reserved name",
            );
        }
        for ordinary in [
            "albums/spring.jpg",
            ".coffretish/spring.jpg",
            "albums/.coffret-fetch-abc.part",
        ] {
            assert!(
                !carries_management_area(&entry_path(ordinary)),
                "{ordinary} is the user's own",
            );
        }
    }
}
