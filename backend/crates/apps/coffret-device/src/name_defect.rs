//! Whether a piece of text is one path component.
//!
//! Two things a person types are held to this shape: the name a Library is
//! called on this device, which becomes a directory name, and a mapping's
//! prefix, which is one top-level component of the Library (spec: EP-9). The
//! check lives beside neither of them, because a check either owned would read
//! as that one deciding the shape of the other.

use crate::error::NameDefect;

/// What is wrong with `name` as one path component, if anything is.
///
/// Only that shape, and nothing about where the text is going: a mapping's
/// prefix names a folder inside the Library rather than a directory beside the
/// other Libraries, so no rule one of the two callers owes alone belongs here.
pub(crate) fn defect_in(name: &str) -> Option<NameDefect> {
    if name.is_empty() {
        return Some(NameDefect::Empty);
    }
    if name == "." || name == ".." {
        return Some(NameDefect::Relative);
    }
    if name.contains('/') || name.contains('\\') {
        return Some(NameDefect::Separator);
    }
    if name.chars().any(char::is_control) {
        return Some(NameDefect::Control);
    }
    None
}
