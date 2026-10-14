/// What the page is served out of: the explorer's build output, by its names
/// relative to the build's root.
///
/// A trait so that a case can serve a build of its own, with the dotfiles and
/// the names a traversal would reach that a real build should never answer.
pub(super) trait Assets {
    /// The contents of the file `name` names, where there is one.
    fn file(&self, name: &str) -> Option<&'static [u8]>;
}

#[cfg(feature = "embed-web")]
impl Assets for include_dir::Dir<'static> {
    fn file(&self, name: &str) -> Option<&'static [u8]> {
        self.get_file(name).map(include_dir::File::contents)
    }
}
