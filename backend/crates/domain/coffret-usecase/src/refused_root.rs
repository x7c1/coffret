use std::fmt;
use std::path::PathBuf;

use coffret_model::{EntryPath, Redacted};

use crate::root_marker::{MalformedMarker, MANAGEMENT_AREA, MARKER_FILE};

/// A mapping whose local root is not the root that was registered
/// (spec: EP-13).
///
/// The companion of [`UnavailableRoot`](crate::UnavailableRoot), and
/// deliberately not the same thing: EP-12 asks whether a root is *there to be
/// read from*, and this asks whether the folder standing at it is the one whose
/// marker this mapping recorded. A root can be perfectly available and still be
/// the wrong folder — a disk mounted where another one used to be, a mapping
/// recorded and the folder then moved — and placing a file into it would put the
/// Library's content somewhere nobody pointed this device at.
///
/// Nothing under such a mapping is placed, and the refusal is reported once for
/// the mapping rather than once per Entry: what went wrong is the root, so
/// naming every file that was not put into it would bury the one fact there is
/// to act on (spec: EP-4's no-silent-selection posture, EP-11's reporting).
///
/// The local root travels in the value because the caller is what decides what
/// to do about it. It never travels into a diagnostic event, and neither does
/// the prefix — an Entry Path component is no more loggable than a local path
/// (spec: EL-1). The reason may: it names no path and no Entry.
///
/// The prefix is nonetheless the half a refusal may *name*, and that is what it
/// is carried for: it is a name inside the Library, chosen by whoever recorded
/// the mapping, rather than a path on this device, so a person-facing refusal
/// says which of a device's mappings it is about (spec: EL-1, EP-13).
///
/// There is deliberately no `PartialEq`, because [`RootRefused`] carries the
/// marker's own refusal and error values here are not compared. `Clone` there
/// is, because a run's outcome is read rather than consumed: whoever renders the
/// findings holds the outcome by reference and copies this out of it.
#[derive(Debug, Clone)]
pub struct RefusedRoot {
    /// The top-level component the mapping stands for, or `None` for the
    /// Library root.
    pub prefix: Option<EntryPath>,
    /// The folder on this device the mapping names.
    pub local_root: PathBuf,
    /// Why the device would not place anything into it.
    pub reason: RootRefused,
}

/// Why a mapped root is not the root the mapping expects (spec: EP-13).
///
/// One variant per case the rule names, and every one of them is a refusal to
/// place rather than something to repair: only recording the mapping ever
/// writes, adopts, or replaces a marker, so a fetch, an upload, and a sync that
/// meet any of these stop and say so. A symbolic link standing at either name is
/// not a case of its own — it is the name not being the required kind, which is
/// what [`ManagementAreaNotADirectory`](Self::ManagementAreaNotADirectory) and
/// [`MarkerNotARegularFile`](Self::MarkerNotARegularFile) say.
///
/// What a person does about each of them is the same gesture, which is why the
/// callers that render one say it once: record the mapping again, and ask for a
/// new identity where the identity is meant to change.
///
/// There is deliberately no `PartialEq`, for the reason the error types around
/// it have none: a caller reports which of these it was and does not compare two
/// of them. `Clone` it has for the reason [`RefusedRoot`] has it.
#[derive(Debug, Clone)]
pub enum RootRefused {
    /// The mapping records no identity for the root to be held against.
    ///
    /// Not a mapping that skips the check: there is nothing for the marker to
    /// agree with, so there is no folder this device may vouch for. A mapping
    /// read back out of a device-state file that predates the marker, or one
    /// written by something other than `coffret map`, arrives this way.
    NoExpectedIdentity,
    /// The root holds no `.coffret` at all.
    ///
    /// The root exists and is not the registered folder, or is the registered
    /// folder with the management area removed. Either way nothing here says
    /// which folder this is.
    ManagementAreaMissing,
    /// Something other than a folder stands at `.coffret`.
    ///
    /// A symbolic link, a file, or anything else: the name is reserved for the
    /// device's own folder (spec: EP-14) and what is standing at it is not one.
    ManagementAreaNotADirectory,
    /// The management area holds no marker file.
    ///
    /// The interrupted registration EP-13 names, or a marker somebody removed.
    MarkerMissing,
    /// Something other than a regular file stands at the marker's name.
    ///
    /// A symbolic link, a folder, a device, or a pipe. Read through it the
    /// identity would be whatever the link points at rather than this root's.
    MarkerNotARegularFile,
    /// The marker's content names no identity.
    MarkerMalformed {
        /// What the reading refused it for.
        cause: MalformedMarker,
    },
    /// The marker names an identity, and it is not the one this mapping
    /// expects.
    ///
    /// The case the whole rule exists for: the folder in front of the device is
    /// a folder some coffret registered, and not the one this mapping was
    /// recorded against.
    MarkerMismatch,
}

impl fmt::Display for RootRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoExpectedIdentity => f.write_str(
                "the mapping records no identity for its root, so there is nothing the marker \
                 could agree with",
            ),
            Self::ManagementAreaMissing => {
                write!(f, "it holds no {MANAGEMENT_AREA} folder")
            }
            Self::ManagementAreaNotADirectory => write!(
                f,
                "{MANAGEMENT_AREA} in it is coffret's own folder and something else is standing \
                 at the name"
            ),
            Self::MarkerMissing => write!(
                f,
                "its {MANAGEMENT_AREA} folder holds no {MARKER_FILE}, which is what an \
                 interrupted registration leaves behind"
            ),
            Self::MarkerNotARegularFile => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in it is not a regular file"
            ),
            Self::MarkerMalformed { cause } => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in it names no identity ({cause})"
            ),
            Self::MarkerMismatch => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in it names another identity, so this is not \
                 the folder the mapping was recorded against"
            ),
        }
    }
}

impl Redacted for RootRefused {
    /// Which refusal it is, and for a malformed marker which defect.
    ///
    /// Every one of them is safe to log: none names the root, the identity it
    /// carries, or anything the user has (spec: EL-1). The identifiers stay out
    /// on purpose all the same — what a diagnostic event is for is how often a
    /// device meets this and which shape it takes, and neither identity adds to
    /// that.
    fn redacted(&self) -> String {
        match self {
            Self::NoExpectedIdentity => "NoExpectedIdentity".to_owned(),
            Self::ManagementAreaMissing => "ManagementAreaMissing".to_owned(),
            Self::ManagementAreaNotADirectory => "ManagementAreaNotADirectory".to_owned(),
            Self::MarkerMissing => "MarkerMissing".to_owned(),
            Self::MarkerNotARegularFile => "MarkerNotARegularFile".to_owned(),
            Self::MarkerMalformed { cause } => {
                format!("MarkerMalformed(defect={})", cause.defect())
            }
            Self::MarkerMismatch => "MarkerMismatch".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_state::RootMarkerId;
    use crate::root_marker;

    // EL-1: a refusal is the one part of this that may be logged, so every
    // variant has to render to something that names neither the folder nor what
    // was in it.
    #[test]
    fn a_refusal_names_the_shape_and_never_the_folder() {
        let malformed =
            root_marker::parse(b"not an identity at all").expect_err("that text names no identity");
        for reason in [
            RootRefused::NoExpectedIdentity,
            RootRefused::ManagementAreaMissing,
            RootRefused::ManagementAreaNotADirectory,
            RootRefused::MarkerMissing,
            RootRefused::MarkerNotARegularFile,
            RootRefused::MarkerMalformed { cause: malformed },
            RootRefused::MarkerMismatch,
        ] {
            let logged = reason.redacted();
            assert!(
                !logged.is_empty() && !logged.contains('/'),
                "a logged refusal names the shape alone, and this one says {logged:?}",
            );
        }
    }

    // The message a person is shown is the other half: it says what is wrong
    // with the folder in front of them rather than naming a variant.
    #[test]
    fn a_mismatch_says_the_folder_is_not_the_one_that_was_recorded() {
        let said = RootRefused::MarkerMismatch.to_string();
        assert!(said.contains(MARKER_FILE), "{said}");
        assert!(said.contains("another identity"), "{said}");

        // And the identity itself is nowhere in it: what a person acts on is
        // the folder, which the caller names, and the gesture, which the caller
        // spells out.
        let id = RootMarkerId::from_bytes([0x11; RootMarkerId::BYTE_LEN]);
        assert!(!said.contains(&id.to_hex()), "{said}");
    }
}
