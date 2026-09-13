use std::fmt;
use std::path::PathBuf;

use coffret_model::EntryPath;

use crate::mapping_named::mapping_named;
use crate::root_refused::RootRefused;

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

/// The sentence a person is shown where this refusal is raised as an error
/// (spec: EP-13).
///
/// Said by the value rather than by each error that carries it:
/// [`FetchError::RefusedRoot`](crate::fetch::FetchError::RefusedRoot), raised by
/// a fetch placing the rest of the Library, and the refusal a device raises
/// while placing one file it was handed, are one state met through two readings.
/// Spelling the sentence out in both would be a chance for one of them to start
/// saying something else about a folder whose whole answer is the same gesture.
///
/// Not the only sentence about this state, and not the one a run that carried on
/// with the device's other mappings lists it under: that one is said in the
/// voice the rest of a run's findings are said in.
///
/// The folder is named because it is the one thing there is to go and look at,
/// the mapping beside it because the gesture is aimed at one of them, and the
/// reason because it says what is wrong with the folder standing there. All
/// three reach a person and none reaches a diagnostic event: rendering this is
/// the deliberate act of putting it in front of whoever asked (spec: EL-1).
impl fmt::Display for RefusedRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is not the folder {} was recorded against: {}; nothing was placed into it, and \
             recording that mapping again is what settles which folder it is",
            self.local_root.display(),
            mapping_named(self.prefix.as_ref()),
            self.reason,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path;

    // What EP-13 asks a refusal to say, in the sentence both readings of it are
    // shown as: the folder to go and look at, which of the device's mappings
    // stands at it, what is wrong with the folder standing there, and the one
    // gesture that settles it. The two halves of EP-9 are said apart, a mapping
    // for a top-level component and the one that stands for the whole Library.
    #[test]
    fn a_refusal_names_the_folder_the_mapping_the_case_and_the_gesture() {
        let refusal = |prefix| RefusedRoot {
            prefix,
            local_root: PathBuf::from("/mnt/copied"),
            reason: RootRefused::MarkerMismatch,
        };

        assert_eq!(
            refusal(Some(entry_path("albums"))).to_string(),
            "/mnt/copied is not the folder the mapping for \"albums\" was recorded against: \
             .coffret/root in it names another identity, so this is not the folder the mapping \
             was recorded against; nothing was placed into it, and recording that mapping again \
             is what settles which folder it is",
        );
        assert_eq!(
            refusal(None).to_string(),
            "/mnt/copied is not the folder the mapping for the Library root was recorded \
             against: .coffret/root in it names another identity, so this is not the folder the \
             mapping was recorded against; nothing was placed into it, and recording that \
             mapping again is what settles which folder it is",
            "the mapping that stands for the whole Library has no component to be named by",
        );
    }
}
