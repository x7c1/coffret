use super::ceiling::check_control_object_len;
use super::encode_request::ControlEncodeRequest;
use super::encoded_object::EncodedControlObject;
use super::header::ControlHeader;
use super::payload;
use crate::aead::{Cipher, TAG_LEN};
use crate::error::{Error, Result};
use crate::nonce;
use crate::purpose::Purpose;

/// Lays out a control object: header, then the payload as one AEAD message.
///
/// The name is checked only for whether it admits the request's kind (FM-12),
/// so nothing is written under a name that would be refused on the way back in.
/// The length is held against the kind's ceiling for the same reason, and before
/// the object is assembled: a Library that has outgrown what a reader will take
/// in should hear so while it is still holding the payload, not after storing an
/// object nothing opens again.
///
/// The nonce is drawn fresh for every object, for the reason
/// [`ControlHeader`] gives.
pub fn encode_control_object(request: &ControlEncodeRequest<'_>) -> Result<EncodedControlObject> {
    let kind = request.kind;
    if !request.name.admits(kind) {
        return Err(Error::ControlObjectKindNotAdmitted {
            name: request.name.clone(),
            kind,
        });
    }
    let key = request.key.require(Purpose::of_control_object(kind))?;

    let nonce = nonce::random()?;
    let header = ControlHeader::new(
        kind,
        request.name.generation(),
        request.name.replica(),
        nonce,
    );
    let header_bytes = header.to_bytes();

    let mut plaintext = payload::encode(request.payload)?;
    let object_len = ControlHeader::LEN + plaintext.len() + TAG_LEN;
    check_control_object_len(kind, object_len as u64)?;
    let mut object = Vec::with_capacity(object_len);
    object.extend_from_slice(&header_bytes);
    Cipher::new(key).seal(&nonce, &header_bytes, &mut plaintext, &mut object)?;

    Ok(EncodedControlObject::new(object, request.name.to_string()))
}
