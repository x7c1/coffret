/// Whether the host is a development one: the development host in front of a
/// checkout, or the development desktop app's, rather than the installed
/// app's.
///
/// A development host answers the favicon's paths with the badged favicon, so
/// that its tabs are told apart from the installed app's running beside it
/// (the page cannot tell which host served it). Everything else is served the
/// same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// The installed app's host: the build served as it is.
    Production,
    /// A development host: the build served with the badged favicon.
    Development,
}
