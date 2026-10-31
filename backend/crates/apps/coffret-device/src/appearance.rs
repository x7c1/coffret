use serde::{Deserialize, Serialize};

/// Which set of colours the explorer is drawn in on this device: the one the
/// system asks for, or one a person chose over it.
///
/// This device's preference about how its explorer looks, and nothing about
/// the Library: it is kept beside the Libraries rather than in one, so every
/// Library this device opens is drawn the same, and nothing of it is ever
/// uploaded. See [`AppearanceFile`](crate::AppearanceFile) for where.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Whichever the system asks for, followed as it changes: what a device
    /// that never chose is drawn in.
    #[default]
    System,
    /// The light set, whatever the system asks for.
    Light,
    /// The dark set, whatever the system asks for.
    Dark,
}
