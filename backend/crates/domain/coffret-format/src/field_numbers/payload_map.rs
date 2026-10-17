use super::map_fields::MapFields;
use super::registry::{INDEX_SNAPSHOT, JOURNAL_RECORD, KEYRING};

/// The map kind a control object of `kind` carries as its payload.
pub fn payload_map(kind: coffret_model::ControlObjectKind) -> &'static MapFields {
    use coffret_model::ControlObjectKind;
    match kind {
        ControlObjectKind::Journal => &JOURNAL_RECORD,
        ControlObjectKind::IndexSnapshot | ControlObjectKind::ActivationSnapshot => &INDEX_SNAPSHOT,
        ControlObjectKind::Keyring => &KEYRING,
    }
}
