use super::object_name::ControlObjectName;
use super::payload::ControlPayload;
use crate::purpose_key::PurposeKey;

/// Everything the encoder needs to lay out one control object.
///
/// The name is the single source of the kind, generation, and replica position:
/// the header is written from it, so a freshly encoded object can never
/// contradict the name it is stored under (FM-12).
#[derive(Debug, Clone)]
pub struct ControlEncodeRequest<'a> {
    /// The name this object will be stored under.
    pub name: &'a ControlObjectName,
    /// The purpose key of the name's kind.
    pub key: &'a PurposeKey,
    /// The payload to seal.
    pub payload: &'a ControlPayload,
}

impl<'a> ControlEncodeRequest<'a> {
    /// A request to write `payload` as the object called `name`.
    pub fn new(
        name: &'a ControlObjectName,
        key: &'a PurposeKey,
        payload: &'a ControlPayload,
    ) -> Self {
        Self { name, key, payload }
    }
}
