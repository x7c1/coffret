use crate::HostKind;

/// The favicon's paths, each beside the file a development host answers it
/// with: the same favicon with a "D" badge, written by `scripts/gen-icons.sh`
/// into the same build. The page links only the plain names, so a development
/// host is told apart by what it answers them with.
const DEVELOPMENT_FAVICONS: [(&str, &str); 3] = [
    ("favicon.svg", "favicon-dev.svg"),
    ("favicon-32.png", "favicon-32-dev.png"),
    ("favicon-32-dark.png", "favicon-32-dark-dev.png"),
];

/// The file a host of `kind` reads to answer the build's name `name`: the
/// badged favicon for a favicon's name on a development host, and `name`
/// itself for everything else.
pub(super) fn file_for(kind: HostKind, name: &str) -> &str {
    match kind {
        HostKind::Production => name,
        HostKind::Development => DEVELOPMENT_FAVICONS
            .iter()
            .find(|(plain, _)| *plain == name)
            .map_or(name, |(_, badged)| badged),
    }
}
