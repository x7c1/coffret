use super::encode_request::ControlEncodeRequest;
use super::encoded_object::EncodedControlObject;
use super::header::ControlHeader;
use super::payload;
use crate::aead::{Cipher, TAG_LEN};
use crate::error::Result;
use crate::nonce;
use crate::purpose::Purpose;

/// Lays out a control object: header, then the payload as one AEAD message.
///
/// The nonce is drawn fresh for every object, for the reason
/// [`ControlHeader`] gives.
pub fn encode_control_object(request: &ControlEncodeRequest<'_>) -> Result<EncodedControlObject> {
    let kind = request.name.kind();
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
    let mut object = Vec::with_capacity(ControlHeader::LEN + plaintext.len() + TAG_LEN);
    object.extend_from_slice(&header_bytes);
    Cipher::new(key).seal(&nonce, &header_bytes, &mut plaintext, &mut object)?;

    Ok(EncodedControlObject::new(object, request.name.to_string()))
}
