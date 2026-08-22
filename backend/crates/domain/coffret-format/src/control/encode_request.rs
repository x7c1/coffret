use coffret_model::{ControlObjectKind, ControlObjectName};

use super::payload::ControlPayload;
use crate::purpose_key::PurposeKey;

/// Everything the encoder needs to lay out one control object.
///
/// The kind and the name are stated separately because a name determines no
/// kind (FM-12). The kind is what goes into the authenticated header and what
/// picks the purpose key; the name only carries the generation and replica
/// position that go in beside it, and the encoder refuses a pairing FM-12's
/// admission table does not list, so a freshly encoded object can never
/// contradict the name it will be stored under.
#[derive(Debug, Clone)]
pub struct ControlEncodeRequest<'a> {
    /// The name this object will be stored under.
    pub name: &'a ControlObjectName,
    /// Which kind of control state the object carries.
    pub kind: ControlObjectKind,
    /// The purpose key of that kind.
    pub key: &'a PurposeKey,
    /// The payload to seal.
    pub payload: &'a ControlPayload,
}

impl<'a> ControlEncodeRequest<'a> {
    /// A request to write `payload` as the object of `kind` called `name`.
    pub fn new(
        name: &'a ControlObjectName,
        kind: ControlObjectKind,
        key: &'a PurposeKey,
        payload: &'a ControlPayload,
    ) -> Self {
        Self {
            name,
            kind,
            key,
            payload,
        }
    }
}
