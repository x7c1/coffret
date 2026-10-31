use coffret_device::Appearance;
use serde::Deserialize;

/// What a page asks to have kept as this device's appearance.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceRequest {
    /// The set chosen: `system`, `light` or `dark`. Anything else is refused
    /// as a body the route cannot read.
    pub(super) appearance: Appearance,
}
