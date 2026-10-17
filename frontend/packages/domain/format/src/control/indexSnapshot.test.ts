import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { MAX_FORMAT_INTEGER } from '../internal/bytes.js';
import type { SnapshotContent } from '../model/snapshotContent.js';
import {
  INDEX_SNAPSHOT_SCHEMA,
  decodeIndexSnapshot,
  encodeIndexSnapshot,
  indexSnapshotKind,
  type IndexSnapshotPayload,
} from './indexSnapshot.js';
import type { ControlPayload } from './payload.js';
import {
  BORN,
  CAPTURED_PATH,
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
  spelledByName,
} from './payloadSchemas.testing.js';
import { INDEX_SNAPSHOT } from '../internal/fieldNumbers.js';

/** An ordinary Snapshot payload with one field changed by hand. */
function tampered(
  change: (map: Map<unknown, unknown>) => void,
  snapshot: IndexSnapshotPayload = ordinary(),
): ControlPayload {
  const payload = encodeIndexSnapshot(snapshot);
  const map = bodyMap(payload, INDEX_SNAPSHOT);
  change(map);
  return withBodyMap(payload.masterKeyEpoch, map, INDEX_SNAPSHOT);
}

function readOrdinary(payload: ControlPayload): IndexSnapshotPayload {
  return decodeIndexSnapshot(payload, 'index-snapshot');
}

/** The Containers of the sample content, in the order the encoder writes them. */
function orderedContainers(): SnapshotContent['containers'] {
  return canonical(content()).containers;
}

describe('Index Snapshot payload (spec: FM-16)', () => {
  // CK-1, CK-2, CK-3: the checkpoint, the Containers, and the Entries come back as
  // they went in, in the order the encoder put them in.
  it('round-trips an ordinary Snapshot', () => {
    const decoded = readOrdinary(encodeIndexSnapshot(ordinary()));
    expect(decoded).toEqual({ content: canonical(content()) });
  });

  // FM-9, FM-16: each Entry carries its entry number in its Container, which the
  // Snapshot's path order would otherwise lose.
  it('carries each entry number', () => {
    const payload = encodeIndexSnapshot(ordinary());
    const entries = arrayField(bodyMap(payload, INDEX_SNAPSHOT), 'entries');
    expect(entries.map((entry) => Number((entry as Map<unknown, unknown>).get('entry')))).toEqual(
      canonical(content()).entries.map((location) => location.entryNumber),
    );
    expect(readOrdinary(payload).content.entries.map((location) => location.entryNumber)).toContain(
      1,
    );
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
    const map = bodyMap(encodeIndexSnapshot(ordinary()), INDEX_SNAPSHOT);
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
    const map = bodyMap(encodeIndexSnapshot(ordinary()), INDEX_SNAPSHOT);
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

  // A Snapshot's entry map is the catalog's spelling, plus the `container`
  // index that is the Snapshot's own — so `path` and `mtime` without the
  // `captured_` prefix FM-9 gives them, and `btime` for the Entries that have
  // one. `albums/spring/a.jpg` sorts first (spec: EP-3), and it is the one
  // with a birth time.
  it('carries the catalog spelling and an optional birth time', () => {
    const payload = encodeIndexSnapshot(ordinary());
    const entries = arrayField(bodyMap(payload, INDEX_SNAPSHOT), 'entries');
    expect([...mapAt(entries, 0).keys()].sort()).toEqual(
      [
        'path',
        'captured_path',
        'offset',
        'size',
        'mtime',
        'btime',
        'hash',
        'container',
        'entry',
      ].sort(),
    );
    for (let index = 1; index < entries.length; index += 1) {
      expect([...mapAt(entries, index).keys()]).not.toContain('btime');
      expect([...mapAt(entries, index).keys()]).not.toContain('captured_path');
    }

    const decoded = readOrdinary(payload);
    expect(decoded.content.entries[0].entry.btimeSeconds).toBe(BORN);
    expect(decoded.content.entries[1].entry.btimeSeconds).toBeUndefined();
  });

  // An Entry's captured path travels beside its current one, each under its
  // own key, and each comes back as the one it went in as.
  it('carries a captured path beside the current path', () => {
    const payload = encodeIndexSnapshot(ordinary());
    const entries = arrayField(bodyMap(payload, INDEX_SNAPSHOT), 'entries');
    expect(mapAt(entries, 0).get('captured_path')).toBe(CAPTURED_PATH);

    const decoded = readOrdinary(payload).content.entries;
    expect(decoded[0].entry.path).toBe('albums/spring/a.jpg');
    expect(decoded[0].entry.capturedPath).toBe(CAPTURED_PATH);
    for (const location of decoded.slice(1)) {
      expect(location.entry.capturedPath).toBe(location.entry.path);
    }
  });

  // `captured_path` is written only where it differs from `path`, so one state
  // has one encoding; a map carrying it equal to `path` is refused.
  it('refuses a captured path equal to the path', () => {
    const payload = tampered((map) => {
      const entries = arrayField(map, 'entries');
      mapAt(entries, 0).set('captured_path', mapAt(entries, 0).get('path'));
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('malformed_index_snapshot');
  });

  // EP-3: the order is over the canonical UTF-8 bytes, which is not the order
  // JavaScript's `<` gives two strings — that compares UTF-16 code units, and a
  // character above U+FFFF sorts below U+E000 there and above it in UTF-8. A
  // Snapshot whose Entries were ordered that way would be refused by the other
  // implementation, so the encoder's order is checked against the bytes here.
  it('orders Entries by their UTF-8 bytes rather than by UTF-16 code units', () => {
    const paths = ['albums/\u{1f4f7}.jpg', 'albums/\ue000.jpg'];
    const source = content();
    source.entries = paths.map((path, index) => located(0x21, index, path, 0n, 10n));
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
      map.set(23, 'whatever');
      for (const name of ['containers', 'entries']) {
        for (const item of arrayField(map, name)) {
          (item as Map<unknown, unknown>).set(24, 1n);
        }
      }
      map.set('schema', 3n);
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

describe('Index Snapshot payloads a reader refuses (spec: FM-16)', () => {
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

  // FM-9, FM-16, FM-19: a Snapshot lists every current Entry with the same
  // values a Container's own entry table records, so a row whose `offset` and
  // `size` end past the last plaintext stream position the format admits places
  // nothing — and the Snapshot does not decode, with the refusal a meta section
  // and a Journal record give the same row.
  it('refuses an entry whose extent lies past the end of the address space', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'entries'), 0).set('offset', MAX_FORMAT_INTEGER);
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('stream_too_long');
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
    source.entries.push(located(0x77, 0, 'zzz/orphan.jpg', 0n, 10n));
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

  // FM-16: a reader refuses any schema below the one this build writes.
  it('refuses a schema below the one before entry numbers', () => {
    for (const schema of [0n, INDEX_SNAPSHOT_SCHEMA - 2n]) {
      const payload = tampered((map) => map.set('schema', schema));
      expect(errorCode(() => readOrdinary(payload)), `schema ${schema}`).toBe(
        'unsupported_index_snapshot_schema',
      );
    }
  });

  // A Snapshot of schema 2 carries no entry numbers, so it is no catalog to
  // start from: it is refused with a refusal of its own, and never numbered from
  // its path order. A reader replays the Journal instead.
  it('does not use a schema 2 Snapshot for entry numbers', () => {
    const payload = tampered((map) => {
      map.set('schema', 2n);
      for (const entry of arrayField(map, 'entries')) {
        (entry as Map<unknown, unknown>).delete('entry');
      }
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('snapshot_predates_entry_numbers');
  });

  // From schema 3 every Snapshot entry carries `entry`.
  it('refuses an entry without its entry number', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'entries'), 1).delete('entry');
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('malformed_index_snapshot');
  });

  // FM-9: an entry number is a u32.
  it('refuses an entry number past 32 bits', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'entries'), 0).set('entry', 0x1_0000_0000n);
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('malformed_index_snapshot');
  });

  // FM-9: a Container and an entry number name one Entry, so two entries
  // carrying the same pair are refused.
  it('refuses two entries naming one Entry', () => {
    const payload = tampered((map) => {
      const entries = arrayField(map, 'entries');
      const first = mapAt(entries, 0);
      const twin = entries
        .slice(1)
        .find((entry) => (entry as Map<unknown, unknown>).get('container') === first.get('container'));
      if (!(twin instanceof Map)) {
        throw new Error('the sample holds two Entries of one Pack');
      }
      twin.set('entry', first.get('entry'));
    });
    expect(errorCode(() => readOrdinary(payload))).toBe('snapshot_names_one_entry_twice');
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

describe('an Index Snapshot keyed by field name (spec: FM-16)', () => {
  // Every field travels under the field number the registry assigns it, and a
  // text key is no field's spelling, so a map carrying its fields only under
  // their names is missing them — refused by the name of the first one asked
  // for.
  it('is refused as missing its fields, by name', () => {
    const read = (): unknown => decodeIndexSnapshot(spelledByName(encodeIndexSnapshot(ordinary()), INDEX_SNAPSHOT), 'index-snapshot');
    expect(errorCode(read)).toBe('malformed_index_snapshot');
    expect(read).toThrow(/schema/);
  });
});
