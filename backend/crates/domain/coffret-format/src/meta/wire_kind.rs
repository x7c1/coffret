use coffret_model::ContainerKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireKind {
    OneFile,
    Pack,
}

impl WireKind {
    /// The text FM-9's `kind` field spells this kind as.
    ///
    /// The serde attribute above spells it for the meta section's map; this
    /// spells it for the control payloads, which build their maps by hand
    /// (FM-15, FM-16). The test below holds the two to one answer.
    pub(crate) const fn spelling(self) -> &'static str {
        match self {
            Self::OneFile => "one-file",
            Self::Pack => "pack",
        }
    }

    /// The kind a spelling names, or `None` for one this format version has no
    /// kind for.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "one-file" => Some(Self::OneFile),
            "pack" => Some(Self::Pack),
            _ => None,
        }
    }
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

#[cfg(test)]
mod tests {
    use ciborium::Value;

    use super::*;

    // Two answers here would put two spellings of one kind on Storage (FM-9).
    #[test]
    fn the_two_spellings_of_one_kind_agree() {
        for kind in [WireKind::OneFile, WireKind::Pack] {
            let value = Value::serialized(&kind).expect("a kind serializes");
            assert_eq!(value.as_text(), Some(kind.spelling()));
            assert_eq!(WireKind::parse(kind.spelling()), Some(kind));
        }
    }

    #[test]
    fn a_spelling_this_format_version_has_no_kind_for_names_none() {
        assert_eq!(WireKind::parse("one_file"), None);
        assert_eq!(WireKind::parse("archive"), None);
    }
}
