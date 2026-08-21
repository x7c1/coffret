use super::LogSettings;

/// The prefix every crate of coffret's own carries in an event's target.
const COFFRET_PREFIX: &str = "coffret_";

/// The crates that are coffret's but are named for a provider, not for coffret.
const GATEWAY_TARGETS: [&str; 2] = ["google_drive_store", "s3_store"];

impl LogSettings {
    /// Whether an event from this target belongs in the file.
    ///
    /// Coffret's own crates by default, and nothing else: the ceiling is finite
    /// and every target shares it, so a dependency narrating its own internals
    /// does not merely add noise — it evicts the evidence the file is kept for.
    /// Anyone investigating that dependency asks for it by name.
    pub(crate) fn keeps(&self, target: &str) -> bool {
        target.starts_with(COFFRET_PREFIX)
            || GATEWAY_TARGETS
                .iter()
                .copied()
                .chain(self.extra_targets.iter().map(String::as_str))
                .any(|allowed| is_within(target, allowed))
    }
}

/// Whether a target is a crate, or something inside it.
///
/// An event's target is a module path, so `s3_store` has to match
/// `s3_store::error` as well as itself — and must not match a crate that merely
/// starts with the same letters.
fn is_within(target: &str, crate_name: &str) -> bool {
    match target.strip_prefix(crate_name) {
        Some(rest) => rest.is_empty() || rest.starts_with("::"),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_is_spent_on_coffrets_own_crates() {
        let settings = LogSettings::new("/tmp/logs");

        assert!(settings.keeps("coffret_usecase::object_store"));
        assert!(settings.keeps("google_drive_store::api::drive_api"));
        assert!(settings.keeps("s3_store::error"));
        // A crate name on its own is a target too, not only a module inside one.
        assert!(settings.keeps("s3_store"));
    }

    #[test]
    fn a_dependency_narrating_itself_does_not_get_the_budget() {
        let settings = LogSettings::new("/tmp/logs");

        // The events that filled a whole run's log before this filter existed.
        assert!(!settings.keeps("aws_smithy_runtime::client::orchestrator"));
        assert!(!settings.keeps("aws_smithy_runtime::client::identity::cache::lazy"));
        assert!(!settings.keeps("hyper_util::client::legacy::connect::http"));
        // A crate that merely starts the same way is somebody else's.
        assert!(!settings.keeps("s3_store_extras::thing"));
    }

    #[test]
    fn a_dependency_asked_for_by_name_is_let_through() {
        let settings = LogSettings::new("/tmp/logs").with_target("aws_smithy_runtime");

        assert!(settings.keeps("aws_smithy_runtime::client::orchestrator"));
        // Only the one that was named.
        assert!(!settings.keeps("hyper_util::client::legacy::connect::http"));
        // And coffret's own are still there.
        assert!(settings.keeps("s3_store::error"));
    }
}
