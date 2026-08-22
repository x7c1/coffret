use coffret_model::ControlObjectKind;
use serde::{Deserialize, Serialize};

/// The control-object kind, spelled as both implementations name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireControlObjectKind {
    /// A Journal record.
    Journal,
    /// A Keyring replica.
    Keyring,
    /// An ordinary Index Snapshot.
    IndexSnapshot,
    /// An Index Snapshot that activates a Master Key epoch.
    ActivationSnapshot,
}

impl From<ControlObjectKind> for WireControlObjectKind {
    fn from(kind: ControlObjectKind) -> Self {
        match kind {
            ControlObjectKind::Journal => Self::Journal,
            ControlObjectKind::Keyring => Self::Keyring,
            ControlObjectKind::IndexSnapshot => Self::IndexSnapshot,
            ControlObjectKind::ActivationSnapshot => Self::ActivationSnapshot,
        }
    }
}

impl From<WireControlObjectKind> for ControlObjectKind {
    fn from(kind: WireControlObjectKind) -> Self {
        match kind {
            WireControlObjectKind::Journal => Self::Journal,
            WireControlObjectKind::Keyring => Self::Keyring,
            WireControlObjectKind::IndexSnapshot => Self::IndexSnapshot,
            WireControlObjectKind::ActivationSnapshot => Self::ActivationSnapshot,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_spellings_are_the_ones_both_implementations_use() {
        assert_eq!(
            serde_json::to_string(&WireControlObjectKind::IndexSnapshot).unwrap(),
            r#""index-snapshot""#
        );
        assert_eq!(
            serde_json::to_string(&WireControlObjectKind::Journal).unwrap(),
            r#""journal""#
        );
        assert_eq!(
            serde_json::to_string(&WireControlObjectKind::Keyring).unwrap(),
            r#""keyring""#
        );
        assert_eq!(
            serde_json::to_string(&WireControlObjectKind::ActivationSnapshot).unwrap(),
            r#""activation-snapshot""#
        );
    }
}
