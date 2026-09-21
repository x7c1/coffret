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
//! The name is compared with ASCII case folded, because a volume that folds
//! case is a volume on which the kernel and a name-side check would otherwise
//! disagree about what "the name" is. The exact spelling is coffret's own; every
//! other spelling that folds to it is somebody's own folder and is refused and
//! reported rather than passed over. Both verdicts live in
//! [`folds_to_management_area`], which says why.
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
///
/// Exactly the reserved name, byte for byte. A spelling that only *folds* to it
/// is not this — it is [`folds_to_management_area`], and the two are kept apart
/// on purpose, because what a caller does with them differs.
pub fn is_management_area(name: &str) -> bool {
    name == MANAGEMENT_AREA
}

/// Whether one local name folds to the reserved name under ASCII case folding
/// without being it (spec: EP-14).
///
/// A filesystem that folds ASCII case — APFS as macOS ships it, an exFAT volume
/// on either platform — does not tell `.COFFRET` apart from `.coffret`, so an
/// open by the reserved name reaches whichever of the two is on disk and hands
/// back a file descriptor, which carries no name. The comparison here is the
/// only place the spelling is still available, and the two verdicts it draws
/// are deliberately asymmetric:
///
/// - The exact name is coffret's own and is stepped over in silence, which is
///   the cost EP-14 states and prices.
/// - Every other spelling that folds to it is **refused and reported**. Folding
///   a seven-letter name admits 128 spellings, so there are 127 of these, and
///   stepping over them too would take any of those folders out of a person's
///   backup on the strength of a cost the register states for one name. A
///   folder somebody named `.COFFRET` for their own reasons is owed a sentence,
///   not an omission they find out about when they need the files back.
///
/// ASCII only. A volume that folds by Unicode rules collides in ways this does
/// not catch; which volumes coffret claims to serve is a question about the
/// product rather than about this comparison.
pub fn folds_to_management_area(name: &str) -> bool {
    !is_management_area(name) && name.eq_ignore_ascii_case(MANAGEMENT_AREA)
}

/// Whether an Entry Path carries the reserved name at any depth (spec: EP-14).
///
/// The path side of the same reservation the scan reads by name. A scan asks
/// [`is_management_area`] of each local name as it walks; a caller holding an
/// Entry Path has no walk to ask it during — the question is settled before a
/// single component is descended — so it asks it of the path's own components
/// instead.
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

/// The first component of an Entry Path that folds to the reserved name without
/// being it, where there is one (spec: EP-14).
///
/// The path side of [`folds_to_management_area`], asked at any depth for the
/// reason [`carries_management_area`] is. The component itself comes back and
/// not merely the fact of it, because what a caller composes from this is a
/// refusal that has to say *which* name is standing there — that is the whole
/// of what makes it a sentence about the person's folder rather than about
/// coffret's.
pub fn component_folding_to_management_area(path: &EntryPath) -> Option<&str> {
    path.as_str()
        .split('/')
        .find(|component| folds_to_management_area(component))
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
            // The text is this layer's finding and what is wrong with the
            // spelling is the reading's, which `source` hands on. A caller
            // printing the chain reads each of them once.
            Self::NotAnIdentity { .. } => {
                f.write_str("a marker's content is the spelling of an identity and this is not")
            }
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

    /// The links a caller printing `{error:#}` reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

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

    // This layer says the content is no spelling of an identity; the reading
    // says what the spelling would have had to be. A caller printing
    // `{error:#}` reads each of those once (spec: EP-13).
    #[test]
    fn a_marker_naming_no_identity_reaches_a_caller_as_one_sentence_per_layer() {
        let refused = parse(b"not an identity").expect_err("that content names no identity");

        assert_eq!(
            chain(&refused),
            vec![
                "a marker's content is the spelling of an identity and this is not".to_owned(),
                format!(
                    "not the {} lowercase hexadecimal characters a root's identity is spelled as",
                    RootMarkerId::HEX_LEN
                ),
                "expected 16 hex characters, found 15".to_owned(),
            ],
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

    // EP-14: the comparison folds ASCII case, and the fold is a verdict of its
    // own rather than a second way of being the reserved name — the exact
    // spelling is coffret's folder, and a spelling that only folds to it is
    // somebody's.
    #[test]
    fn a_spelling_that_folds_to_the_reserved_name_is_not_the_reserved_name() {
        for folded in [".COFFRET", ".Coffret", ".cOfFrEt"] {
            assert!(
                folds_to_management_area(folded),
                "{folded} folds to the reserved name",
            );
            assert!(
                !is_management_area(folded),
                "{folded} is not the reserved name itself",
            );
        }

        // The exact name is exactly one of the two, and never both: a caller
        // that asked the fold first would refuse coffret's own folder.
        assert!(!folds_to_management_area(MANAGEMENT_AREA));

        for ordinary in [".COFFRETISH", "COFFRET", ".COFFRET-FETCH-ABC.PART"] {
            assert!(
                !folds_to_management_area(ordinary),
                "{ordinary} is the user's own and folds to nothing reserved",
            );
        }
    }

    // The path side of the same fold, at any depth, and it hands back the
    // spelling rather than a yes: a refusal composed from this has to name the
    // folder standing there.
    #[test]
    fn a_component_folding_to_the_reserved_name_is_found_at_any_depth() {
        assert_eq!(
            component_folding_to_management_area(&entry_path("albums/.COFFRET/spring.jpg")),
            Some(".COFFRET"),
        );
        assert_eq!(
            component_folding_to_management_area(&entry_path(".Coffret")),
            Some(".Coffret"),
        );
        // The exact name carries no fold, so the two questions never both
        // answer for one path.
        assert_eq!(
            component_folding_to_management_area(&entry_path("albums/.coffret/root")),
            None,
        );
        assert_eq!(
            component_folding_to_management_area(&entry_path("albums/spring.jpg")),
            None,
        );
    }
}
