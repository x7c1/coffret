use std::fmt;

use crate::library_dir::STAGING_SUFFIX;

/// What is wrong with the name a Library has on this device.
///
/// The name becomes a directory name beside the other Libraries', so it is one
/// path component as this device spells one. It is not an Entry Path and shares
/// no vocabulary with one: a backslash and a control character are refused here
/// and carried without comment inside the Library, because a name here is what a
/// person navigates their own disk by.
#[derive(Debug)]
pub enum NameDefect {
    /// Nothing was given.
    Empty,
    /// It holds a path separator, so it names more than one component.
    Separator,
    /// It is `.` or `..`, which name a directory rather than sit in one.
    Relative,
    /// It holds a control character, which no name should carry.
    Control,
    /// It ends in the suffix a Library being created is staged under, so it
    /// would collide with another Library's half-built directory.
    StagingSuffix,
}

impl fmt::Display for NameDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("it is empty"),
            Self::Separator => f.write_str("it holds a path separator"),
            Self::Relative => f.write_str("it names a directory rather than sits in one"),
            Self::Control => f.write_str("it holds a control character"),
            // The suffix is named rather than described: a person told their
            // name "ends in the suffix a Library is staged under" has been told
            // which name is refused but not which ending to drop.
            Self::StagingSuffix => write!(
                f,
                "it ends in {STAGING_SUFFIX:?}, which is what a Library being created is staged \
                 under"
            ),
        }
    }
}
