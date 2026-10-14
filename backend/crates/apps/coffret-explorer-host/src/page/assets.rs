use super::Lookup;

/// What the page is served out of: the explorer's build output, by its names
/// relative to the build's root.
///
/// A trait so that the build compiled in, a build on disk, and a case's own
/// build — with the dotfiles and the names a traversal would reach that a real
/// build should never answer — are served by one set of rules.
pub(super) trait Assets {
    /// The file `name` names, where there is one. `name` has already been held
    /// to naming no dotfile and stepping nowhere outside the build.
    fn file(&self, name: &str) -> Lookup;
}

#[cfg(feature = "embed-web")]
impl Assets for include_dir::Dir<'static> {
    fn file(&self, name: &str) -> Lookup {
        match self.get_file(name) {
            Some(file) => Lookup::Found(file.contents().into()),
            None => Lookup::Missing,
        }
    }
}
