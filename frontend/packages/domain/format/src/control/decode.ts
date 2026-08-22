import { open } from '../internal/aead.js';
import { fail } from '../errors.js';
import { purposeKeyBytes, purposeOfControlObject, type PurposeKey } from '../purposeKey.js';
import { CONTROL_HEADER_LENGTH, parseControlHeader } from './header.js';
import { decodeControlPayload, type ControlPayload } from './payload.js';
import { nameAdmitsKind, parseControlObjectName } from './objectName.js';
import type { Generation } from '../model/generation.js';
import type { ReplicaPosition } from '../model/replicaPosition.js';
import type { ControlObjectKind } from '../model/kinds.js';

/** An opened control object: what its header said, and what it carried. */
export interface DecodedControlObject {
  /** Which kind of control state this object carries. */
  kind: ControlObjectKind;
  /** Where the object sat in the Library's control history (FM-13). */
  generation: Generation;
  /** Which replica this is, out of how many. */
  replica: ReplicaPosition;
  /** The payload, with the epoch that wrote it. */
  payload: ControlPayload;
}

/**
 * Opens a control object stored under `objectName`.
 *
 * The name is part of what is checked, not decoration: recovery discovers these
 * objects by name, while what an object *is* rides in its authenticated header.
 * A name that did not lead to the object it promised is therefore a
 * disagreement about what the object is, and the object is rejected (FM-12).
 * The kind is checked against FM-12's admission table rather than for equality,
 * because one name form covers the whole control-head chain —
 * `head-<generation>` admits an ordinary Journal record and the Index Snapshot
 * that activates an epoch, and nothing else. The generation and the replica
 * position are the name's alone to state, so those are checked for equality.
 * All of it is on plaintext bytes, before the key is used at all.
 */
export function decodeControlObject(
  object: Uint8Array,
  objectName: string,
  key: PurposeKey,
): DecodedControlObject {
  const name = parseControlObjectName(objectName);
  const header = parseControlHeader(object);
  if (!nameAdmitsKind(name, header.kind)) {
    fail(
      'control_object_kind_not_admitted',
      `${JSON.stringify(objectName)} admits no control object of kind ${header.kind}`,
    );
  }
  if (!name.generation.equals(header.generation)) {
    fail('object_name_mismatch', 'the object name and its header disagree on generation');
  }
  if (!name.replica.equals(header.replica)) {
    fail('object_name_mismatch', 'the object name and its header disagree on replica position');
  }

  const keyBytes = purposeKeyBytes(key, purposeOfControlObject(header.kind));
  const associatedData = object.subarray(0, CONTROL_HEADER_LENGTH);
  const message = object.subarray(CONTROL_HEADER_LENGTH);
  if (message.length === 0) {
    fail('missing_control_payload', 'control object carries no payload');
  }

  // The associated data is the header exactly as it appears in the object.
  const plaintext = open(keyBytes, header.nonce, associatedData, message);
  return {
    kind: header.kind,
    generation: header.generation,
    replica: header.replica,
    payload: decodeControlPayload(plaintext),
  };
}
