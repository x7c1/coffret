use std::borrow::Cow;

/// What looking a name up in the explorer's build came to.
pub(super) enum Lookup {
    /// The file the name names, and its contents.
    Found(Cow<'static, [u8]>),
    /// No file by that name: a path the explorer reads itself, which is
    /// answered with the index.
    Missing,
    /// A name that reaches outside the build, such as through a symbolic
    /// link, or a file that could not be read: answered with the fixed `404`.
    Refused,
}
