use super::decoded_object::DecodedControlObject;
use super::header::ControlHeader;
use super::object_name::ControlObjectName;
use super::payload;
use crate::aead::Cipher;
use crate::error::{Error, Result};
use crate::purpose::Purpose;
use crate::purpose_key::PurposeKey;

/// Opens a control object stored under `object_name`.
///
/// The name is part of what is checked, not decoration: recovery finds these
/// objects by name, so a name that disagrees with the header inside it is a
/// disagreement about what the object *is* and the object is rejected. Both the
/// name and the header are checked on plaintext bytes, before the key is used at
/// all.
pub fn decode_control_object(
    object: &[u8],
    object_name: &str,
    key: &PurposeKey,
) -> Result<DecodedControlObject> {
    let name = ControlObjectName::parse(object_name)?;
    let header = ControlHeader::parse(object)?;
    if name.kind() != header.kind {
        return Err(Error::ObjectNameMismatch { field: "kind" });
    }
    if name.generation() != header.generation {
        return Err(Error::ObjectNameMismatch {
            field: "generation",
        });
    }
    if name.replica() != header.replica {
        return Err(Error::ObjectNameMismatch {
            field: "replica position",
        });
    }

    let key = key.require(Purpose::of_control_object(header.kind))?;
    let (associated_data, message) = object.split_at(ControlHeader::LEN);
    if message.is_empty() {
        return Err(Error::MissingControlPayload);
    }

    // The associated data is the header exactly as it appears in the object.
    let plaintext = Cipher::new(key).open(header.nonce(), associated_data, message)?;
    Ok(DecodedControlObject {
        kind: header.kind,
        generation: header.generation,
        replica: header.replica,
        payload: payload::decode(&plaintext)?,
    })
}
