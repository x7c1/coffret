import { TAG_LENGTH, seal } from '../internal/aead.js';
import { concatBytes } from '../internal/bytes.js';
import { randomNonce } from '../internal/nonce.js';
import { purposeKeyBytes, purposeOfControlObject, type PurposeKey } from '../purposeKey.js';
import { requireControlObjectLength } from './ceiling.js';
import { CONTROL_HEADER_LENGTH, encodeControlHeader } from './header.js';
import { encodeControlPayload, type ControlPayload } from './payload.js';
import {
  formatControlObjectName,
  nameAdmitsKind,
  type ControlObjectName,
} from './objectName.js';
import { fail } from '../errors.js';
import type { ControlObjectKind } from '../model/kinds.js';

/**
 * Everything the encoder needs to lay out one control object.
 *
 * The kind and the name are stated separately because a name determines no kind
 * (FM-12). The kind is what goes into the authenticated header and what picks
 * the purpose key; the name only carries the generation and replica position
 * that go in beside it, and the encoder refuses a pairing FM-12's admission
 * table does not list, so a freshly encoded object can never contradict the name
 * it will be stored under.
 */
export interface ControlEncodeRequest {
  /** The name this object will be stored under. */
  name: ControlObjectName;
  /** Which kind of control state the object carries. */
  kind: ControlObjectKind;
  /** The purpose key of that kind. */
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
 * The name is checked only for whether it admits the request's kind (FM-12), so
 * nothing is written under a name that would be refused on the way back in.
 *
 * The length is held against the kind's ceiling for the same reason, and before
 * the object is assembled (FM-11): a Library that has outgrown what a reader
 * will take in should hear so while it is still holding the payload, not after
 * storing an object nothing opens again.
 *
 * The nonce is drawn fresh for every object, for the reason the header's
 * documentation gives.
 */
export function encodeControlObject(request: ControlEncodeRequest): EncodedControlObject {
  const { kind } = request;
  const { generation, replica } = request.name;
  if (!nameAdmitsKind(request.name, kind)) {
    fail(
      'control_object_kind_not_admitted',
      `${JSON.stringify(formatControlObjectName(request.name))} admits no control object of kind ${kind}`,
    );
  }
  const keyBytes = purposeKeyBytes(request.key, purposeOfControlObject(kind));

  const nonce = request.nonce ?? randomNonce();
  const header = encodeControlHeader({ kind, generation, replica, nonce });
  const plaintext = encodeControlPayload(request.payload);
  // The object is the header and one AEAD message, so this is its length before
  // a byte of it is laid out.
  requireControlObjectLength(kind, CONTROL_HEADER_LENGTH + plaintext.length + TAG_LENGTH);

  return {
    bytes: concatBytes(header, seal(keyBytes, nonce, header, plaintext)),
    objectName: formatControlObjectName(request.name),
  };
}
