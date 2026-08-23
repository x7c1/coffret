import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import type { SnapshotContent } from '../model/snapshotContent.js';
import {
  decodeIndexSnapshot,
  encodeIndexSnapshot,
  indexSnapshotKind,
  type IndexSnapshotPayload,
} from './indexSnapshot.js';
import type { ControlPayload } from './payload.js';
import {
  activating,
  arrayField,
  bodyMap,
  canonical,
  containerId,
  content,
  located,
  mapAt,
  ordinary,
  summary,
  withBodyMap,
} from './payloadSchemas.testing.js';

/** An ordinary Snapshot payload with one field changed by hand. */
function tampered(
  change: (map: Map<unknown, unknown>) => void,
  snapshot: IndexSnapshotPayload = ordinary(),
): ControlPayload {
  const payload = encodeIndexSnapshot(snapshot);
  const map = bodyMap(payload);
  change(map);
  return withBodyMap(payload.masterKeyEpoch, map);
}

function readOrdinary(payload: ControlPayload): IndexSnapshotPayload {
  return decodeIndexSnapshot(payload, 'index-snapshot');
}

/** The Containers of the sample content, in the order the encoder writes them. */
function orderedContainers(): SnapshotContent['containers'] {
  return canonical(content()).containers;
}

describe('Index Snapshot payload (FM-16)', () => {
  // CK-1 to CK-3: the checkpoint, the Containers, and the Entries come back as
  // they went in, in the order the encoder put them in.
  it('round-trips an ordinary Snapshot', () => {
    const decoded = readOrdinary(encodeIndexSnapshot(ordinary()));
    expect(decoded).toEqual({ content: canonical(content()) });
  });

  // MR-2: an activation Snapshot carries the same content and, beyond it, which
  // head it fenced and the slot it won.
  it('round-trips an activation Snapshot', () => {
    const payload = encodeIndexSnapshot(activating());
    const decoded = decodeIndexSnapshot(payload, 'activation-snapshot');
    expect(decoded.activation).toEqual(activating().activation);
    expect(decoded.content).toEqual(canonical(content()));
  });

  // CP-2, CP-15: a name-keyed Storage persists no token, so an activation
  // Snapshot from one carries a base head generation and no slot — and that
  // absence is not what tells the two Snapshot kinds apart.
  it('round-trips an activation Snapshot without a minted slot', () => {
    const source = activating();
    delete source.activation?.activationSlot;
    const decoded = decodeIndexSnapshot(encodeIndexSnapshot(source), 'activation-snapshot');
    expect(decoded.activation?.activationSlot).toBeUndefined();
    expect(decoded.activation?.baseHeadGeneration.value).toBe(
      source.activation?.baseHeadGeneration.value,
    );
  });

  // The kind an object is framed as follows from the payload rather than from a
  // flag beside it.
  it('says which kind it has to be framed as', () => {
    expect(indexSnapshotKind(ordinary())).toBe('index-snapshot');
    expect(indexSnapshotKind(activating())).toBe('activation-snapshot');
  });

  // CK-7: a Snapshot carries no device state, so nothing records which
  // checkpoint an Index adopted — there is no field for it at all.
  it('carries no field naming what an Index adopted', () => {
    const map = bodyMap(encodeIndexSnapshot(ordinary()));
    for (const key of map.keys()) {
      expect(String(key)).not.toContain('adopted');
    }
  });

  // One Library state has one encoding, whatever order the Index reported it in.
  it('encodes the same content identically whatever order it was reported in', () => {
    const reordered = content();
    reordered.containers.reverse();
    reordered.entries.reverse();
    expect(encodeIndexSnapshot({ content: reordered }).body).toEqual(
      encodeIndexSnapshot(ordinary()).body,
    );
  });

  // An Entry names its Container by index, so the 16-byte ID appears once per
  // Container rather than once per Entry.
  it('names an Entry’s Container by index', () => {
    const map = bodyMap(encodeIndexSnapshot(ordinary()));
    const containers = orderedContainers();
    const expected = canonical(content()).entries.map((location) =>
      containers.findIndex((container) => container.id.toHex() === location.containerId.toHex()),
    );

    // A writer may spell a small integer as a CBOR number or a bignum, so the
    // comparison is on the value rather than on the runtime type it decoded to.
    const entries = arrayField(map, 'entries');
    expect(entries.map((_, index) => Number(mapAt(entries, index).get('container')))).toEqual(
      expected,
    );
    for (let index = 0; index < entries.length; index += 1) {
      expect(mapAt(entries, index).has('id')).toBe(false);
    }
  });

  // EP-3: the order is over the canonical UTF-8 bytes, which is not the order
  // JavaScript's `<` gives two strings — that compares UTF-16 code units, and a
  // character above U+FFFF sorts below U+E000 there and above it in UTF-8. A
  // Snapshot whose Entries were ordered that way would be refused by the other
  // implementation, so the encoder's order is checked against the bytes here.
  it('orders Entries by their UTF-8 bytes rather than by UTF-16 code units', () => {
    const paths = ['albums/\u{1f4f7}.jpg', 'albums/\ue000.jpg'];
    const source = content();
    source.entries = paths.map((path) => located(0x21, path, 0n, 10n));
    source.containers = [summary(0x21, 'one-file')];

    const utf8 = new TextEncoder();
    const expected = [...paths].sort((left, right) => {
      const one = utf8.encode(left);
      const other = utf8.encode(right);
      for (let index = 0; index < Math.min(one.length, other.length); index += 1) {
        if (one[index] !== other[index]) {
          return one[index] - other[index];
        }
      }
      return one.length - other.length;
    });

    const decoded = readOrdinary(encodeIndexSnapshot({ content: source }));
    expect(decoded.content.entries.map((location) => location.entry.path)).toEqual(expected);
  });

  // FM-9: the maps are forward-open at every level.
  it('ignores fields it does not know', () => {
    const payload = tampered((map) => {
      map.set('future_field', 'whatever');
      for (const name of ['containers', 'entries']) {
        for (const item of arrayField(map, name)) {
          (item as Map<unknown, unknown>).set('future_element_field', 1n);
        }
      }
      map.set('schema', 2n);
    });
    expect(readOrdinary(payload).content).toEqual(canonical(content()));
  });

  // A Snapshot of a Library that holds nothing is still a Snapshot: it has a
  // checkpoint to preserve.
  it('round-trips a Snapshot of an empty Library', () => {
    const empty = content();
    empty.containers = [];
    empty.entries = [];
    const decoded = readOrdinary(encodeIndexSnapshot({ content: empty }));
    expect(decoded.content.containers).toEqual([]);
    expect(decoded.content.entries).toEqual([]);
  });

  // What makes a payload unreadable is an Entry without a Container, not a
  // Container without an Entry.
  it('keeps a Container no Entry names', () => {
    const source = content();
    source.containers.push(summary(0xf0, 'pack'));
    const decoded = readOrdinary(encodeIndexSnapshot({ content: source }));
    const empty = containerId(0xf0).toHex();
    expect(decoded.content.containers.some((container) => container.id.toHex() === empty)).toBe(
      true,
    );
    expect(decoded.content.entries.some((location) => location.containerId.toHex() === empty)).toBe(
      false,
    );
  });
});

describe('Index Snapshot payloads a reader refuses (FM-16)', () => {
  it('refuses Containers out of ID order', () => {
    const payload = tampered((map) =>
      map.set('containers', arrayField(map, 'containers').reverse()),
    );
    expect(errorCode(() => readOrdinary(payload))).toBe('control_payload_out_of_order');
  });

  // EP-3: the order is what lets a prefix range be answered by binary search, so
  // a payload out of it would answer such a range wrongly rather than slowly.
  it('refuses Entries out of Entry Path order', () => {
    const payload = tampered((map) => map.set('entries', arrayField(map, 'entries').reverse()));
    expect(errorCode(() => readOrdinary(payload))).toBe('control_payload_out_of_order');
  });

  // EP-5: one Entry Path holds at most one current Entry.
  it('refuses one Entry Path listed twice', () => {
    const payload = tampered((map) => {
      const entries = arrayField(map, 'entries');
      entries[1] = entries[0];
      map.set('entries', entries);
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('control_payload_out_of_order');
  });

  // An index past the end of `containers` names no Container at all, so the
  // Snapshot cannot be read back into an Index.
  it('refuses an Entry naming a Container past the end', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'entries'), 0).set('container', 9n);
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('dangling_container_index');
  });

  // The same, on the way out: content whose Entry is held by a Container the
  // Snapshot does not list has no index to write.
  it('refuses to write an Entry whose Container is not listed', () => {
    const source = content();
    source.entries.push(located(0x77, 'zzz/orphan.jpg', 0n, 10n));
    expect(errorCode(() => encodeIndexSnapshot({ content: source }))).toBe(
      'snapshot_entry_without_container',
    );
  });

  // A commitment is a plain interface here, so an encoder that took one on trust
  // would write a Snapshot every reader refuses.
  it('refuses to write a Keyring commitment no reader would take', () => {
    const source = content();
    const checkpoint = { ...source.checkpoint, keyring: { ...source.checkpoint.keyring } };
    checkpoint.keyring.setDigest = 'BEEF';
    expect(errorCode(() => encodeIndexSnapshot({ content: { ...source, checkpoint } }))).toBe(
      'invalid_set_digest',
    );
  });

  // MR-2: the activation fields are the activation kind's alone, so an ordinary
  // Snapshot carrying one contradicts its own authenticated header.
  it('refuses an ordinary Snapshot carrying activation fields', () => {
    for (const field of ['base_head_generation', 'activation_slot']) {
      const payload = tampered((map) =>
        map.set(field, field === 'activation_slot' ? 'minted-head-7' : 6n),
      );
      expect(errorCode(() => readOrdinary(payload)), field).toBe(
        'activation_field_on_ordinary_snapshot',
      );
    }
  });

  // The other direction: an object whose header says it activated an epoch has
  // to say which head it fenced, or nothing records the fence at all.
  it('refuses an activation Snapshot without the head it fenced', () => {
    const payload = tampered((map) => map.delete('base_head_generation'), activating());
    expect(errorCode(() => decodeIndexSnapshot(payload, 'activation-snapshot'))).toBe(
      'activation_snapshot_field_missing',
    );
  });

  it('refuses an ordinary payload read as an activation Snapshot', () => {
    const payload = encodeIndexSnapshot(ordinary());
    expect(errorCode(() => decodeIndexSnapshot(payload, 'activation-snapshot'))).toBe(
      'activation_snapshot_field_missing',
    );
  });

  // FM-11: only two of the four control-object kinds are Index Snapshots.
  it('refuses a kind that is no Index Snapshot', () => {
    const payload = encodeIndexSnapshot(ordinary());
    for (const kind of ['journal', 'keyring'] as const) {
      expect(errorCode(() => decodeIndexSnapshot(payload, kind)), kind).toBe(
        'not_an_index_snapshot_kind',
      );
    }
  });

  it('refuses a schema below one', () => {
    const payload = tampered((map) => map.set('schema', 0n));
    expect(errorCode(() => readOrdinary(payload))).toBe('unsupported_index_snapshot_schema');
  });

  it('refuses a missing checkpoint field', () => {
    const payload = tampered((map) => map.delete('journal_generation'));
    expect(errorCode(() => readOrdinary(payload))).toBe('malformed_index_snapshot');
  });

  it('refuses a Container of an unknown kind', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'containers'), 0).set('kind', 'archive');
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('malformed_index_snapshot');
  });
});
