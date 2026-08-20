use std::fmt;

use coffret_model::ControlObjectKind;

/// What a key derived from the Master Key is allowed to encrypt.
///
/// The Master Key is never an AEAD key itself: every use passes through HKDF
/// with the purpose's info string, so a key derived for one purpose is useless
/// for another and adding a purpose is adding an info string. The strings
/// spelled here are format constants — changing one would orphan every object
/// already written under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Purpose {
    /// Container Keys, wrapped into Key Envelopes.
    ContainerWrap,
    /// Journal record payloads.
    ControlJournal,
    /// Keyring replica payloads.
    ControlKeyring,
    /// Index Snapshot payloads.
    ControlIndexSnapshot,
    /// The OAuth token cache a device keeps for a Storage provider.
    ///
    /// The only purpose so far whose key protects device-local state rather
    /// than a Storage Object: the cache never reaches Storage, and has no
    /// control-object kind to be reached through.
    TokenCache,
}

impl Purpose {
    /// The info string this purpose is derived under.
    pub const fn info(self) -> &'static str {
        match self {
            Self::ContainerWrap => "coffret/v1/container-wrap",
            Self::ControlJournal => "coffret/v1/control/journal",
            Self::ControlKeyring => "coffret/v1/control/keyring",
            Self::ControlIndexSnapshot => "coffret/v1/control/index-snapshot",
            Self::TokenCache => "coffret/v1/token-cache",
        }
    }

    /// The purpose that encrypts payloads of the given control-object kind.
    ///
    /// Exhaustive over [`ControlObjectKind`] and nothing else: a purpose that
    /// encrypts no control object — [`Purpose::TokenCache`] — is reached by
    /// naming it, not through a kind invented to stand for it.
    pub const fn of_control_object(kind: ControlObjectKind) -> Self {
        match kind {
            ControlObjectKind::Journal => Self::ControlJournal,
            ControlObjectKind::Keyring => Self::ControlKeyring,
            ControlObjectKind::IndexSnapshot => Self::ControlIndexSnapshot,
        }
    }
}

impl fmt::Display for Purpose {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.info())
    }
}

/// Every purpose the v1 registry lists, for tests that must cover them all.
#[cfg(test)]
pub(crate) const ALL: [Purpose; 5] = [
    Purpose::ContainerWrap,
    Purpose::ControlJournal,
    Purpose::ControlKeyring,
    Purpose::ControlIndexSnapshot,
    Purpose::TokenCache,
];

#[cfg(test)]
mod tests {
    use super::*;

    // KD-4: the v1 purpose registry and the info string each purpose derives
    // under.
    #[test]
    fn info_strings_match_the_registry() {
        assert_eq!(Purpose::ContainerWrap.info(), "coffret/v1/container-wrap");
        assert_eq!(Purpose::ControlJournal.info(), "coffret/v1/control/journal");
        assert_eq!(Purpose::ControlKeyring.info(), "coffret/v1/control/keyring");
        assert_eq!(
            Purpose::ControlIndexSnapshot.info(),
            "coffret/v1/control/index-snapshot"
        );
        assert_eq!(Purpose::TokenCache.info(), "coffret/v1/token-cache");
    }

    // KD-4: a key derived for one purpose is used for no other, so no two
    // purposes may share an info string.
    #[test]
    fn every_purpose_has_its_own_info_string() {
        for (i, left) in ALL.iter().enumerate() {
            for right in &ALL[i + 1..] {
                assert_ne!(left.info(), right.info());
            }
        }
    }

    // KD-4, FM-11: each control-object kind is encrypted under its own purpose.
    #[test]
    fn each_control_object_kind_maps_to_its_own_purpose() {
        assert_eq!(
            Purpose::of_control_object(ControlObjectKind::Journal),
            Purpose::ControlJournal
        );
        assert_eq!(
            Purpose::of_control_object(ControlObjectKind::Keyring),
            Purpose::ControlKeyring
        );
        assert_eq!(
            Purpose::of_control_object(ControlObjectKind::IndexSnapshot),
            Purpose::ControlIndexSnapshot
        );
    }
}
