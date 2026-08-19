use coffret_model::ContainerKind;
use serde::{Deserialize, Serialize};

/// The Container kind, spelled as the meta section's `kind` field spells it.
///
/// Every user-data Container records one explicit kind, which is never inferred
/// from the Entry count (PK-15), so the manifest states it as its own field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WireContainerKind {
    /// A one-file Container, the kind uploading one file on its own creates.
    OneFile,
    /// A Pack, the kind `freeze`, repack, and compaction create.
    Pack,
}

impl From<ContainerKind> for WireContainerKind {
    fn from(kind: ContainerKind) -> Self {
        match kind {
            ContainerKind::OneFile => Self::OneFile,
            ContainerKind::Pack => Self::Pack,
        }
    }
}

impl From<WireContainerKind> for ContainerKind {
    fn from(kind: WireContainerKind) -> Self {
        match kind {
            WireContainerKind::OneFile => Self::OneFile,
            WireContainerKind::Pack => Self::Pack,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both implementations write the kind with the spellings FM-9 gives the
    // meta section's `kind` field, so the manifest uses them too.
    #[test]
    fn the_kind_spellings_are_the_wire_ones() {
        assert_eq!(
            serde_json::to_string(&WireContainerKind::OneFile).unwrap(),
            r#""one-file""#
        );
        assert_eq!(
            serde_json::to_string(&WireContainerKind::Pack).unwrap(),
            r#""pack""#
        );
    }
}
