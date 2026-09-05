/**
 * The map a control-object payload records one Container with.
 *
 * A Journal record's addition and an Index Snapshot's `containers` element are
 * the same five fields — `id`, `kind`, `ciphertext_hash`, `ciphertext_len`, and
 * optional `object_ref` (FM-15, FM-16) — because they say the same thing: what
 * the Library knows about a current Container without opening it (CP-11). An
 * addition carries the Container's entry table beside them, which the Journal
 * record's own module adds to the map this one builds.
 */

import { takeExactly } from '../internal/bytes.js';
import {
  optionalText,
  requiredBytes,
  requiredText,
  requiredUint,
  setUint,
  type CborMap,
} from '../internal/cbor.js';
import { fail, type CoffretErrorCode } from '../errors.js';
import { CONTAINER_ID_LENGTH, ContainerId } from '../model/containerId.js';
import { CONTENT_HASH_LENGTH } from '../model/entry.js';
import { isContainerKind } from '../model/kinds.js';
import type { ContainerSummary } from '../model/containerSummary.js';

/** The five fields, ready for a caller to add its own to. */
export function encodeContainerMap(container: ContainerSummary): Map<string, unknown> {
  const map = new Map<string, unknown>([
    ['id', container.id.bytes()],
    ['kind', container.kind],
    [
      'ciphertext_hash',
      takeExactly(container.ciphertextHash, CONTENT_HASH_LENGTH, 'a ciphertext hash'),
    ],
  ]);
  setUint(map, 'ciphertext_len', container.ciphertextLength, 'control_payload_encode_failed');
  if (container.objectRef !== undefined) {
    map.set('object_ref', container.objectRef);
  }
  return map;
}

/** Reads the five fields out of a map that may carry more. */
export function decodeContainerMap(map: CborMap, code: CoffretErrorCode): ContainerSummary {
  const kind = requiredText(map, 'kind', code);
  if (!isContainerKind(kind)) {
    fail(code, `kind names no Container kind: ${JSON.stringify(kind)}`);
  }
  const container: ContainerSummary = {
    id: ContainerId.fromBytes(
      takeExactly(requiredBytes(map, 'id', code), CONTAINER_ID_LENGTH, 'a Container ID'),
    ),
    kind,
    ciphertextHash: takeExactly(
      requiredBytes(map, 'ciphertext_hash', code),
      CONTENT_HASH_LENGTH,
      'a ciphertext hash',
    ),
    ciphertextLength: requiredUint(map, 'ciphertext_len', code),
  };
  const objectRef = optionalText(map, 'object_ref', code);
  if (objectRef !== undefined) {
    container.objectRef = objectRef;
  }
  return container;
}
