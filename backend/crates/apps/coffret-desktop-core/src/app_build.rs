//! Which of the two apps this is: the installed app, or the development app
//! `scripts/desktop.sh` builds beside it (`make desktop-dev`).
//!
//! Told apart by the app's identifier, and read by everything that marks the
//! development app so it is told apart from the installed app running beside
//! it: the desktop app's tray icon and tooltip, and the explorer's favicon,
//! which the host serves badged for a development app
//! ([`AppBuild::host_kind`]).

use coffret_explorer_host::HostKind;

/// The development app's identifier, which `scripts/desktop.sh` builds it with
/// in place of `tauri.conf.json`'s.
const DEV_IDENTIFIER: &str = "io.github.x7c1.coffret.dev";

/// Which of the two apps this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppBuild {
    Installed,
    Development,
}

impl AppBuild {
    /// The build the app's identifier names.
    pub fn of(identifier: &str) -> Self {
        if identifier == DEV_IDENTIFIER {
            Self::Development
        } else {
            Self::Installed
        }
    }

    /// The kind of host the explorer is served from by this build.
    pub fn host_kind(self) -> HostKind {
        match self {
            Self::Installed => HostKind::Production,
            Self::Development => HostKind::Development,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_development_identifier_is_the_development_app() {
        assert_eq!(AppBuild::of(DEV_IDENTIFIER), AppBuild::Development);
        assert_eq!(
            AppBuild::of(DEV_IDENTIFIER).host_kind(),
            HostKind::Development
        );
    }

    #[test]
    fn any_other_identifier_is_the_installed_app() {
        assert_eq!(AppBuild::of("io.github.x7c1.coffret"), AppBuild::Installed);
        assert_eq!(
            AppBuild::of("io.github.x7c1.coffret").host_kind(),
            HostKind::Production
        );
    }
}
