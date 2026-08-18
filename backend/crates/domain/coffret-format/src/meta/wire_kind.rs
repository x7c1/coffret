use coffret_model::ContainerKind;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum WireKind {
    OneFile,
    Pack,
}

impl From<ContainerKind> for WireKind {
    fn from(kind: ContainerKind) -> Self {
        match kind {
            ContainerKind::OneFile => Self::OneFile,
            ContainerKind::Pack => Self::Pack,
        }
    }
}

impl From<WireKind> for ContainerKind {
    fn from(kind: WireKind) -> Self {
        match kind {
            WireKind::OneFile => Self::OneFile,
            WireKind::Pack => Self::Pack,
        }
    }
}
