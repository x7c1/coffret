//! The folders a desktop keeps its trash in at the top of a volume.
//!
//! A mapped root may be a volume's own top — a disk mounted at the folder the
//! person mapped — and a desktop moving a file to the trash on that volume
//! keeps the trash there: `.Trash-<uid>` or `.Trash/<uid>` under the
//! freedesktop.org Trash specification, `.Trashes` on macOS, and `.Trash` again
//! for the trash in a home folder mapped as a whole. A sync's move of a departed
//! file (spec: EP-15) lands there too. None of it is Library content, so a scan
//! never enters such a folder and a fetch never places a file in it
//! (spec: EP-16).
//!
//! Only as the first component under a mapped root, because that is the one
//! place a desktop puts them; a folder of that name anywhere deeper is the
//! person's own. Exactly these spellings, compared byte for byte: a desktop
//! writes them so, and a fold of one is a folder somebody named.
//!
//! Nothing here does any I/O, for the reason [`root_marker`](crate::root_marker)
//! and [`scratch`](crate::scratch) beside it do none: the scan and the fetch ask
//! one question of a name, and it has one answer.

/// The trash folder macOS keeps at a volume's top.
const VOLUME_TRASH: &str = ".Trashes";

/// The freedesktop.org Trash's shared folder at a volume's top, and the name of
/// the trash in a home folder on macOS.
const TRASH: &str = ".Trash";

/// What the freedesktop.org Trash's per-user folder at a volume's top begins
/// with, the user's numeric identity following it.
const PER_USER_TRASH_PREFIX: &str = ".Trash-";

/// Whether a name standing directly under a mapped root is a folder a desktop
/// keeps its trash in rather than content to back up (spec: EP-16).
///
/// The caller decides that the name *is* directly under a mapped root: this
/// answers only from the name.
pub fn is_trash_folder(name: &str) -> bool {
    if name == TRASH || name == VOLUME_TRASH {
        return true;
    }
    name.strip_prefix(PER_USER_TRASH_PREFIX)
        .is_some_and(|uid| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // EP-16: the three spellings a desktop writes at a volume's top, the
    // per-user one with any numeric identity.
    #[test]
    fn the_names_a_desktop_keeps_its_trash_under_are_trash_folders() {
        for name in [
            ".Trash",
            ".Trashes",
            ".Trash-1000",
            ".Trash-0",
            ".Trash-501",
        ] {
            assert!(is_trash_folder(name), "{name} is a trash folder");
        }
    }

    // EP-16: exactly those spellings, so a name that merely resembles one is the
    // person's own folder and is backed up as usual.
    #[test]
    fn a_name_that_only_resembles_one_is_the_persons_own() {
        for name in [
            ".trash",
            ".TRASHES",
            "Trash",
            ".Trash-",
            ".Trash-abc",
            ".Trash-1000x",
            ".Trash-1000 ",
            ".Trashcan",
            ".Trashes-1000",
        ] {
            assert!(!is_trash_folder(name), "{name} is the person's own");
        }
    }
}
