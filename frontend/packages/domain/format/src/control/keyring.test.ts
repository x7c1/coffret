import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { CONTAINER_ID_LENGTH } from '../model/containerId.js';
import { Generation } from '../model/generation.js';
import { KEY_ENVELOPE_LENGTH } from '../model/keyEnvelope.js';
import { ReplicaPosition } from '../model/replicaPosition.js';
import type { KeyringMapping } from '../model/keyringMapping.js';
import { compareBytes } from './canonicalOrder.js';
import { decodeKeyring, encodeKeyring, keyringDigestInput, keyringSetDigest } from './keyring.js';
import { keyringReplicaName } from './objectName.js';
import type { ControlPayload } from './payload.js';
import {
  EPOCH,
  arrayField,
  bodyMap,
  containerId,
  envelope,
  mapAt,
  mapping,
  pinnedMapping,
  withBodyMap,
} from './payloadSchemas.testing.js';

/**
 * The digest of {@link pinnedMapping}, which the Rust suite pins too.
 *
 * Both implementations compute this from the same two entries, so a change to
 * what FM-17 hashes — the field order inside an element, the array order, the
 * CBOR spelling of a length — moves it here and in `round_trip_tests.rs` at
 * once. A digest that moved in only one of them is exactly the drift the interop
 * exchange exists to catch, caught before the exchange runs.
 */
const PINNED_SET_DIGEST = '6e6018ce7522ab4f82f4e43d51463efa48a0f57b1862d67b1a439c3d329c783a';

/** The mapping as the encoder puts it on the wire: entries in ID order. */
function canonical(source: KeyringMapping): KeyringMapping {
  return {
    entries: [...source.entries].sort((left, right) =>
      compareBytes(left.containerId.bytes(), right.containerId.bytes()),
    ),
  };
}

/** A Keyring payload with one thing changed by hand, as a reader meets it. */
function tampered(change: (map: Map<unknown, unknown>) => void): ControlPayload {
  const payload = encodeKeyring(mapping(), EPOCH);
  const map = bodyMap(payload);
  change(map);
  return withBodyMap(payload.masterKeyEpoch, map);
}

/** The fields of one element of `mapping`, for a case that has to change them. */
function element(map: Map<unknown, unknown>, index: number): Map<unknown, unknown> {
  return mapAt(arrayField(map, 'mapping'), index);
}

describe('Keyring payload (FM-17)', () => {
  // FM-17, KL-7: both of the things a Keyring holds for a Container — an
  // envelope and the explicit key-lost marker — come back as they went in, in
  // the Container ID order the encoder put them in.
  it('round-trips a mapping of envelopes and a marker', () => {
    expect(decodeKeyring(encodeKeyring(mapping(), EPOCH))).toEqual(canonical(mapping()));
  });

  // A Library holding no Container yet still has a Keyring generation to
  // commit: the mapping is empty, not missing.
  it('round-trips an empty mapping', () => {
    expect(decodeKeyring(encodeKeyring({ entries: [] }, EPOCH)).entries).toEqual([]);
  });

  // FM-17: one mapping has one encoding, whatever order a caller held it in —
  // which is what makes the digest a property of the mapping rather than of the
  // writer.
  it('encodes the same mapping identically whatever order it was held in', () => {
    const reordered = mapping();
    reordered.entries.reverse();
    expect(encodeKeyring(reordered, EPOCH).body).toEqual(encodeKeyring(mapping(), EPOCH).body);
    expect(keyringSetDigest(reordered)).toBe(keyringSetDigest(mapping()));
  });

  // KL-1, KL-14: the digest is a function of the mapping alone, so it is the
  // same value every device computes for one generation — and it is pinned,
  // because moving it silently would leave every name and commitment already
  // written naming a set no reader can now match.
  it('computes the pinned digest of the pinned mapping', () => {
    expect(keyringSetDigest(pinnedMapping())).toBe(PINNED_SET_DIGEST);
  });

  // FM-17: what the digest covers is deterministic CBOR, spelled out here byte
  // by byte rather than taken from the encoder — the pin above is only worth
  // having if the bytes behind it are the ones the rule names.
  //
  // Definite lengths, shortest-form arguments, and each element's keys in
  // encoded order: `id` (two characters) before `envelope` or `key_lost`
  // (eight).
  it('hashes the mapping array as deterministic CBOR', () => {
    const expected = Uint8Array.from([
      0x82, // an array of two elements
      0xa2, // a map of two pairs
      0x62,
      ...utf8('id'),
      0x50, // a byte string of 16
      ...new Uint8Array(CONTAINER_ID_LENGTH).fill(0x11),
      0x68,
      ...utf8('envelope'),
      0x58,
      0x48, // a byte string of 72
      ...new Uint8Array(KEY_ENVELOPE_LENGTH).fill(0x22),
      0xa2, // the marker's element is a map of two pairs as well
      0x62,
      ...utf8('id'),
      0x50,
      ...new Uint8Array(CONTAINER_ID_LENGTH).fill(0x33),
      0x68,
      ...utf8('key_lost'),
      0xf5, // true
    ]);
    // Copied into a plain `Uint8Array`: what the encoder hands back is a view
    // of whatever byte buffer the runtime gave it, and only its contents are
    // being asserted here.
    expect(Uint8Array.from(keyringDigestInput(pinnedMapping()))).toEqual(expected);
  });

  // FM-17: the digest covers the mapping, so it cannot also be inside it. The
  // payload carries `mapping` and `schema` and nothing else.
  it('leaves the digest out of the payload', () => {
    expect([...bodyMap(encodeKeyring(mapping(), EPOCH)).keys()]).toEqual(['schema', 'mapping']);
  });

  // FM-12: the digest is the lowercase hex token a replica's name is built
  // from, so the name builder takes what this returns without any further
  // spelling.
  it('produces the token a replica name carries', () => {
    const digest = keyringSetDigest(mapping());
    const name = keyringReplicaName(Generation.of(12n), digest, ReplicaPosition.of(1, 3));
    expect(name.setDigest).toBe(digest);
  });

  // FM-11, FM-13: the generation and the replica position are the header's and
  // the epoch is the framing's field, so none of the three is repeated in the
  // map. The epoch still travels, on the payload the framing hands back.
  it('leaves the generation, the replica, and the epoch to the framing', () => {
    const payload = encodeKeyring(mapping(), EPOCH);
    const map = bodyMap(payload);
    for (const absent of ['generation', 'replica_index', 'replica_count', 'epoch']) {
      expect(map.has(absent), absent).toBe(false);
    }
    expect(payload.masterKeyEpoch.value).toBe(EPOCH.value);
  });

  // FM-9: the maps are forward-open, so a field a newer writer added is stepped
  // over — at the payload's own level and inside an element.
  it('ignores unknown fields', () => {
    const payload = tampered((map) => {
      map.set('future_field', 'whatever');
      map.set('schema', 2n);
      for (const [index] of arrayField(map, 'mapping').entries()) {
        element(map, index).set('future_element_field', 1n);
      }
    });
    expect(decodeKeyring(payload)).toEqual(canonical(mapping()));
  });

  // FM-17, KL-7: an envelope says the Container opens and the marker says no
  // envelope is reachable. An element carrying both says both, which is not a
  // state a Container can be in.
  it('rejects an element with both an envelope and a marker', () => {
    const payload = tampered((map) => element(map, 0).set('key_lost', true));
    expect(errorCode(() => decodeKeyring(payload))).toBe('keyring_entry_with_envelope_and_marker');
  });

  // The other way round: an element that says nothing about its Container maps
  // it to no determinate state, and a mapping of such elements could not be the
  // complete one KL-7 obliges.
  it('rejects an element with neither an envelope nor a marker', () => {
    const payload = tampered((map) => element(map, 0).delete('envelope'));
    expect(errorCode(() => decodeKeyring(payload))).toBe(
      'keyring_entry_without_envelope_or_marker',
    );
  });

  // FM-17: the marker is spelled `true`, so `false` is a writer stating the
  // field in a form the rule does not define — not a way of saying there is no
  // marker.
  it('rejects a key-lost marker that is not true', () => {
    const payload = tampered((map) => element(map, 2).set('key_lost', false));
    expect(errorCode(() => decodeKeyring(payload))).toBe('keyring_entry_marker_not_true');
  });

  // FM-17: `mapping` is in Container ID order so that one mapping has one
  // encoding and therefore one `set_digest`. A payload out of that order is
  // refused rather than sorted: sorting it would accept a second encoding of
  // one state, whose digest no name and no commitment matches.
  it('rejects a mapping out of Container ID order', () => {
    const payload = tampered((map) => arrayField(map, 'mapping').reverse());
    expect(errorCode(() => decodeKeyring(payload))).toBe('control_payload_out_of_order');
  });

  // KL-7: one Container has one entry in the mapping, so an ID listed twice is
  // not a sorted mapping with a repeat in it — it is a payload holding two
  // answers about one Container.
  it('rejects one Container mapped twice', () => {
    const payload = tampered((map) => {
      const entries = arrayField(map, 'mapping');
      entries[1] = entries[0];
    });
    expect(errorCode(() => decodeKeyring(payload))).toBe('control_payload_out_of_order');
  });

  // FM-14: an envelope is 72 bytes, and a field of another length carried the
  // shape the schema gives it but not a value the type accepts.
  it('rejects an envelope that is not the length FM-14 gives it', () => {
    const payload = tampered((map) =>
      element(map, 0).set('envelope', new Uint8Array(KEY_ENVELOPE_LENGTH - 1).fill(0x11)),
    );
    expect(errorCode(() => decodeKeyring(payload))).toBe('invalid_byte_length');
  });

  it('rejects a schema below one', () => {
    const payload = tampered((map) => map.set('schema', 0n));
    expect(errorCode(() => decodeKeyring(payload))).toBe('unsupported_keyring_schema');
  });

  it('rejects a payload with no mapping', () => {
    const payload = tampered((map) => map.delete('mapping'));
    expect(errorCode(() => decodeKeyring(payload))).toBe('malformed_keyring_payload');
  });

  it('rejects an element that is not a map', () => {
    const payload = tampered((map) => {
      arrayField(map, 'mapping')[0] = envelope(0x40).bytes();
    });
    expect(errorCode(() => decodeKeyring(payload))).toBe('malformed_keyring_payload');
  });

  // KL-7: an envelope and a marker are different answers about one Container,
  // and a reader keeps them apart rather than collapsing a marker into "no
  // envelope".
  it('reads a marker back as a marker and not as an absence', () => {
    const decoded = decodeKeyring(encodeKeyring(mapping(), EPOCH));
    const lost = decoded.entries.find((entry) =>
      entry.containerId.equals(containerId(0x99)),
    );
    expect(lost?.key).toEqual({ status: 'key-lost' });
  });
});

function utf8(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}
