import { seal } from '../internal/aead.js';
import { concatBytes } from '../internal/bytes.js';
import { randomNonce } from '../internal/nonce.js';
import { purposeKeyBytes, purposeOfControlObject, type PurposeKey } from '../purposeKey.js';
import { encodeControlHeader } from './header.js';
import { encodeControlPayload, type ControlPayload } from './payload.js';
import { formatControlObjectName, type ControlObjectName } from './objectName.js';

/**
 * Everything the encoder needs to lay out one control object.
 *
 * The caller states the kind, generation, and replica position once, as the
 * name the object will be stored under, and the header is written from that
 * same value. Those three ride authoritatively in the authenticated header; the
 * name only carries them so recovery can discover the object. Writing both from
 * one value is what keeps a freshly encoded object from contradicting the name
 * it is stored under (FM-12).
 */
export interface ControlEncodeRequest {
  /** The name this object will be stored under. */
  name: ControlObjectName;
  /** The purpose key of the name's kind. */
  key: PurposeKey;
  /** The payload to seal. */
  payload: ControlPayload;
  /** The nonce to seal under; drawn from the CSPRNG when left out. */
  nonce?: Uint8Array;
}

/**
 * A finished control object: the bytes to upload and the name to upload them
 * under.
 */
export interface EncodedControlObject {
  /** The full object, header first. */
  bytes: Uint8Array;
  /** The name this object is stored under. */
  objectName: string;
}

/**
 * Lays out a control object: header, then the payload as one AEAD message.
 *
 * The nonce is drawn fresh for every object, for the reason the header's
 * documentation gives.
 */
export function encodeControlObject(request: ControlEncodeRequest): EncodedControlObject {
  const { kind, generation, replica } = request.name;
  const keyBytes = purposeKeyBytes(request.key, purposeOfControlObject(kind));

  const nonce = request.nonce ?? randomNonce();
  const header = encodeControlHeader({ kind, generation, replica, nonce });
  const plaintext = encodeControlPayload(request.payload);

  return {
    bytes: concatBytes(header, seal(keyBytes, nonce, header, plaintext)),
    objectName: formatControlObjectName(request.name),
  };
}
