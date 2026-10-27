import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { MAX_FORMAT_INTEGER } from '../internal/bytes.js';
import { Generation } from '../model/generation.js';
import type { JournalRecord } from '../model/journalRecord.js';
import {
  JOURNAL_RECORD_LOWEST_READ_SCHEMA,
  JOURNAL_RECORD_REQUIRED_SCHEMA_WRITTEN,
  JOURNAL_RECORD_SCHEMA,
  decodeJournalRecord,
  encodeJournalRecord,
} from './journalRecord.js';
import type { ControlPayload } from './payload.js';
import {
  BORN,
  EPOCH,
  GENERATION,
  CAPTURED_DIR,
  addition,
  arrayField,
  bodyMap,
  containerId,
  firstRecord,
  mapAt,
  record,
  renamedRecord,
  withBodyMap,
  spelledByName,
} from './payloadSchemas.testing.js';
import { JOURNAL_RECORD } from '../internal/fieldNumbers.js';

/** The record as the encoder puts it on the wire: arrays in Container ID order. */
function canonical(source: JournalRecord): JournalRecord {
  return {
    ...source,
    additions: [...source.additions].sort((left, right) =>
      left.container.id.toHex() < right.container.id.toHex() ? -1 : 1,
    ),
    removals: [...source.removals].sort((left, right) => (left.toHex() < right.toHex() ? -1 : 1)),
    renames: [...source.renames].sort((left, right) =>
      left.containerId.toHex() === right.containerId.toHex()
        ? left.entryNumber - right.entryNumber
        : left.containerId.toHex() < right.containerId.toHex()
          ? -1
          : 1,
    ),
  };
}

/** A record payload with one field changed by hand, as a reader meets it. */
function tampered(
  change: (map: Map<unknown, unknown>) => void,
  source: JournalRecord = record(),
): ControlPayload {
  const payload = encodeJournalRecord(source);
  const map = bodyMap(payload, JOURNAL_RECORD);
  change(map);
  return withBodyMap(payload.masterKeyEpoch, map, JOURNAL_RECORD);
}

function read(payload: ControlPayload): JournalRecord {
  return decodeJournalRecord(payload, GENERATION);
}

describe('Journal record payload (spec: FM-15)', () => {
  // The Keyring tuple it commits to, both slots it reserves, the Containers it
  // added with their entry tables, and the ones it removed all come back as
  // they went in.
  it('round-trips a record with everything', () => {
    const source = record();
    expect(read(encodeJournalRecord(source))).toEqual(canonical(source));
  });

  // Each rename comes back as it went in — the Container, the entry number and
  // the new path, and nothing else, since the old name is the catalog's — in
  // the order the encoder put them in.
  it('round-trips a record with renames', () => {
    const source = renamedRecord();
    const payload = encodeJournalRecord(source);
    expect(read(payload)).toEqual(canonical(source));

    const renames = arrayField(bodyMap(payload, JOURNAL_RECORD), 'renames');
    expect([...mapAt(renames, 0).keys()]).toEqual(['container', 'entry', 'path']);
    expect(renames.map((rename) => Number((rename as Map<unknown, unknown>).get('entry')))).toEqual([
      0, 3, 1,
    ]);
  });

  // A record that renames nothing leaves `renames` out rather than writing it
  // empty.
  it('writes no renames field for a record that renames nothing', () => {
    const payload = encodeJournalRecord(record());
    expect([...bodyMap(payload, JOURNAL_RECORD).keys()]).not.toContain('renames');
  });

  // FM-9, FM-15: `deposited_at` is must-understand, so every record names the
  // schema that carries it, 4, in `required_schema`, whatever else it carries —
  // a name included, which is not must-understand.
  it('names the schema deposit times need on every record', () => {
    for (const source of [
      record(),
      renamedRecord(),
      firstRecord(),
      { ...firstRecord(), libraryName: 'books' },
    ]) {
      const payload = encodeJournalRecord(source);
      expect(BigInt(bodyMap(payload, JOURNAL_RECORD).get('required_schema') as number)).toBe(
        JOURNAL_RECORD_REQUIRED_SCHEMA_WRITTEN,
      );
    }
    expect(JOURNAL_RECORD_REQUIRED_SCHEMA_WRITTEN).toBe(4n);
  });

  // FM-15: a record that names the Library carries the name under
  // `library_name` and reads back with it; one naming nothing writes no such
  // field; one of schema 4, written before records could carry a name, reads as
  // one naming nothing; and an empty name is refused.
  it('carries the Library name where the record gives one', () => {
    const named: JournalRecord = { ...firstRecord(), libraryName: 'Photos 写真' };
    const payload = encodeJournalRecord(named);
    expect(decodeJournalRecord(payload, Generation.FIRST)).toEqual(canonical(named));

    const unnamed = encodeJournalRecord(firstRecord());
    expect([...bodyMap(unnamed, JOURNAL_RECORD).keys()]).not.toContain('library_name');

    const older = tampered((map) => map.set('schema', 4n), firstRecord());
    expect(decodeJournalRecord(older, Generation.FIRST).libraryName).toBeUndefined();

    const emptied = tampered((map) => map.set('library_name', ''), named);
    expect(errorCode(() => decodeJournalRecord(emptied, Generation.FIRST))).toBe(
      'malformed_journal_record',
    );
  });

  // FM-15: every Entry an addition carries has its own deposit time, written
  // under `deposited_at` and read back Entry for Entry.
  it('carries a deposit time for every addition entry', () => {
    const source = record();
    const payload = encodeJournalRecord(source);
    for (const item of arrayField(bodyMap(payload, JOURNAL_RECORD), 'additions')) {
      for (const entry of arrayField(item as Map<unknown, unknown>, 'entries')) {
        expect([...(entry as Map<unknown, unknown>).keys()]).toContain('deposited_at');
      }
    }
    const decoded = read(payload);
    for (const addition of decoded.additions) {
      expect(addition.depositTimes).toHaveLength(addition.entries.length);
    }
    expect(decoded).toEqual(canonical(source));
  });

  // FM-9, FM-15: a record naming a schema above the one this package
  // implements is refused as needing a newer build, never read in part.
  it('refuses a record requiring a newer reader as needing a newer build', () => {
    for (const source of [record(), renamedRecord()]) {
      const payload = tampered(
        (map) => map.set('required_schema', JOURNAL_RECORD_SCHEMA + 1n),
        source,
      );
      expect(errorCode(() => read(payload))).toBe('journal_record_needs_newer_build');
      expect(() => read(payload)).toThrow(/newer build/);
    }
  });

  // A record of this schema names the schema its deposit times need, so naming
  // none, or one below it, is malformed.
  it('refuses a required schema below the one deposit times need', () => {
    const bare = tampered((map) => map.delete('required_schema'));
    expect(errorCode(() => read(bare))).toBe('malformed_journal_record');
    for (const required of [0n, 2n, JOURNAL_RECORD_LOWEST_READ_SCHEMA - 1n]) {
      const payload = tampered((map) => map.set('required_schema', required));
      expect(errorCode(() => read(payload)), `required ${required}`).toBe(
        'malformed_journal_record',
      );
    }
  });

  // An addition's entry table is the catalog's own spelling — `path`, `mtime`,
  // and an optional `btime`, without the `captured_` prefix FM-9 gives the meta
  // section's copy of the same values. A record listing them under FM-9's keys
  // would be one no replay could read.
  it('writes an entry table in the catalog spelling', () => {
    const payload = encodeJournalRecord(record());
    // The one addition whose table holds two Entries, once the encoder has put
    // `additions` in Container ID order.
    const entries = arrayField(mapAt(arrayField(bodyMap(payload, JOURNAL_RECORD), 'additions'), 1), 'entries');
    expect([...mapAt(entries, 1).keys()].sort()).toEqual(
      ['path', 'offset', 'size', 'mtime', 'btime', 'hash', 'derived_from', 'mime', 'deposited_at'].sort(),
    );
  });

  // An Entry's captured path travels beside its current one, each under its
  // own key, and each comes back as the one it went in as.
  it('carries a captured path beside the current path', () => {
    const payload = encodeJournalRecord(record());
    const entries = arrayField(mapAt(arrayField(bodyMap(payload, JOURNAL_RECORD), 'additions'), 1), 'entries');
    expect(mapAt(entries, 0).get('path')).toBe('albums/40/cover.jpg');
    expect(mapAt(entries, 0).get('captured_path')).toBe(`${CAPTURED_DIR}/40/cover.jpg`);

    const decoded = read(payload).additions[1].entries;
    expect(decoded[0].path).toBe('albums/40/cover.jpg');
    expect(decoded[0].capturedPath).toBe(`${CAPTURED_DIR}/40/cover.jpg`);
    expect(decoded[1].capturedPath).toBe(decoded[1].path);
    expect([...mapAt(entries, 1).keys()]).not.toContain('captured_path');
  });

  // `btime` is optional, so a record carries it for the Entries whose files had
  // one and writes no key at all for the rest — and a replay gets both answers
  // back the way they went in.
  it('carries a birth time only for the entry that has one', () => {
    const payload = encodeJournalRecord(record());
    const entries = arrayField(mapAt(arrayField(bodyMap(payload, JOURNAL_RECORD), 'additions'), 1), 'entries');
    expect([...mapAt(entries, 0).keys()]).not.toContain('btime');

    const decoded = read(payload);
    expect(decoded.additions[1].entries[0].btimeSeconds).toBeUndefined();
    expect(decoded.additions[1].entries[1].btimeSeconds).toBe(BORN);
  });

  // CP-2, CP-15: at generation 0 there is no predecessor, and a name-keyed
  // Storage persists no slot token, so all three optional fields are absent —
  // and absent is not the same as present-and-empty on the way back.
  it('round-trips a record with no predecessor and no minted slots', () => {
    const source = firstRecord();
    const decoded = decodeJournalRecord(encodeJournalRecord(source), Generation.FIRST);
    expect(decoded).toEqual(canonical(source));
    expect(decoded.prev).toBeUndefined();
    expect(decoded.nextCommitSlot).toBeUndefined();
    expect(decoded.snapshotSlot).toBeUndefined();
  });

  // The optional fields are left out of the map rather than written as
  // something empty, so a reader never has two spellings of "nothing".
  it('leaves absent optional fields out of the map', () => {
    const map = bodyMap(encodeJournalRecord(firstRecord()), JOURNAL_RECORD);
    for (const absent of ['prev', 'next_commit_slot', 'snapshot_slot']) {
      expect(map.has(absent), absent).toBe(false);
    }
  });

  // FM-15: `prev` is the record's own statement of the head it was built on, so
  // a record at generation g states g - 1 and nothing else. A reader that took
  // the object's name for the chain would replay a record at a position its
  // authenticated payload never claimed.
  it('rejects a prev that is not the previous generation', () => {
    const payload = tampered((map) => map.set('prev', GENERATION.value - 3n));
    expect(errorCode(() => read(payload))).toBe('journal_record_prev_mismatch');
  });

  // FM-15: only the record at generation 0 was built on nothing, so a later one
  // carrying no `prev` states no head at all.
  it('rejects a record above generation zero without prev', () => {
    const payload = tampered((map) => map.delete('prev'));
    expect(errorCode(() => read(payload))).toBe('journal_record_prev_mismatch');
  });

  // FM-13: the Library's first head succeeds nothing, so a `prev` on it names a
  // head that never existed.
  it('rejects a prev on the first record', () => {
    const encoded = encodeJournalRecord(firstRecord());
    const map = bodyMap(encoded, JOURNAL_RECORD);
    map.set('prev', 0n);
    const payload = withBodyMap(encoded.masterKeyEpoch, map, JOURNAL_RECORD);
    expect(errorCode(() => decodeJournalRecord(payload, Generation.FIRST))).toBe(
      'journal_record_prev_mismatch',
    );
  });

  // One Library state has one encoding, whatever order the record was held in.
  it('encodes the same content identically whatever order it was held in', () => {
    const reordered = record();
    reordered.additions.reverse();
    reordered.removals.reverse();
    expect(encodeJournalRecord(reordered).body).toEqual(encodeJournalRecord(record()).body);
  });

  // FM-13: the generation is the header's and the epoch is the framing's, so
  // neither is repeated in the map.
  it('takes the generation and the epoch from the framing', () => {
    const payload = encodeJournalRecord(record());
    const map = bodyMap(payload, JOURNAL_RECORD);
    expect(map.has('generation')).toBe(false);
    expect(map.has('master_key_epoch')).toBe(false);

    const decoded = read(payload);
    expect(decoded.generation.value).toBe(GENERATION.value);
    expect(decoded.masterKeyEpoch.value).toBe(EPOCH.value);
  });

  // A commit that only removes Containers adds none.
  it('round-trips a record that only removes', () => {
    const source = record();
    source.additions = [];
    const decoded = read(encodeJournalRecord(source));
    expect(decoded.additions).toEqual([]);
    expect(decoded.removals).toHaveLength(2);
  });

  // CP-14: a removal is the Container ID and nothing else.
  it('writes a removal as the Container ID alone', () => {
    const removals = arrayField(bodyMap(encodeJournalRecord(record()), JOURNAL_RECORD), 'removals');
    expect(removals).toEqual([containerId(0x11).bytes(), containerId(0x99).bytes()]);
  });

  // PK-15: the kind is the explicit one the Container recorded, never one
  // guessed from how many Entries the addition carries.
  it('keeps a Pack holding one Entry a Pack', () => {
    const source = firstRecord();
    source.additions = [addition(0x30, 'one-file')];
    source.additions[0].container.kind = 'pack';
    const decoded = decodeJournalRecord(encodeJournalRecord(source), Generation.FIRST);
    expect(decoded.additions[0].entries).toHaveLength(1);
    expect(decoded.additions[0].container.kind).toBe('pack');
  });

  // FM-9: the maps are forward-open, so a field a newer writer added is stepped
  // over rather than refused.
  it('ignores fields it does not know', () => {
    const payload = tampered((map) => {
      map.set(23, 'whatever');
      for (const item of arrayField(map, 'additions')) {
        (item as Map<unknown, unknown>).set(24, 1n);
      }
      map.set('schema', JOURNAL_RECORD_SCHEMA + 1n);
    });
    expect(read(payload)).toEqual(canonical(record()));
  });
});

describe('Journal record payloads a reader refuses (spec: FM-15)', () => {
  // The order is what makes one state have one encoding, so a payload out of it
  // is refused rather than sorted.
  it('refuses additions out of Container ID order', () => {
    const payload = tampered((map) => map.set('additions', arrayField(map, 'additions').reverse()));
    expect(errorCode(() => read(payload))).toBe('control_payload_out_of_order');
  });

  it('refuses removals out of Container ID order', () => {
    const payload = tampered((map) => map.set('removals', arrayField(map, 'removals').reverse()));
    expect(errorCode(() => read(payload))).toBe('control_payload_out_of_order');
  });

  // One Container is added once, so a record naming one twice is not a record
  // in order with a repeat in it.
  it('refuses a Container added twice', () => {
    const payload = tampered((map) => {
      const additions = arrayField(map, 'additions');
      additions[1] = additions[0];
      map.set('additions', additions);
    });
    expect(errorCode(() => read(payload))).toBe('control_payload_out_of_order');
  });

  // FM-15: a reader refuses any schema below the lowest it reads: every earlier
  // schema's additions carry no deposit time.
  it('refuses a schema below the lowest readable one', () => {
    for (const schema of [0n, 2n, JOURNAL_RECORD_LOWEST_READ_SCHEMA - 1n]) {
      const payload = tampered((map) => map.set('schema', schema));
      expect(errorCode(() => read(payload)), `schema ${schema}`).toBe(
        'unsupported_journal_record_schema',
      );
    }
  });

  // FM-15: the lowest schema a reader accepts is its own floor rather than the
  // schema this package writes, so a record at that floor — in `schema` and in
  // `required_schema` — is read, and the next lower one is refused.
  it('reads a record at the lowest readable schema and refuses one below it', () => {
    const atFloor = tampered((map) => {
      map.set('schema', JOURNAL_RECORD_LOWEST_READ_SCHEMA);
      map.set('required_schema', JOURNAL_RECORD_LOWEST_READ_SCHEMA);
    });
    expect(read(atFloor)).toEqual(canonical(record()));

    const below = tampered((map) => map.set('schema', JOURNAL_RECORD_LOWEST_READ_SCHEMA - 1n));
    expect(errorCode(() => read(below))).toBe('unsupported_journal_record_schema');
    const requiringBelow = tampered((map) =>
      map.set('required_schema', JOURNAL_RECORD_LOWEST_READ_SCHEMA - 1n),
    );
    expect(errorCode(() => read(requiringBelow))).toBe('malformed_journal_record');
  });

  // `renames` is ordered by Container ID and then entry number, across
  // Containers and within one, and a payload out of it is refused rather than
  // sorted.
  it('refuses renames out of order', () => {
    for (const [one, other] of [
      [1, 2],
      [0, 1],
    ]) {
      const payload = tampered((map) => {
        const renames = arrayField(map, 'renames');
        [renames[one], renames[other]] = [renames[other], renames[one]];
      }, renamedRecord());
      expect(errorCode(() => read(payload)), `swap ${one} ${other}`).toBe(
        'control_payload_out_of_order',
      );
    }
  });

  // `(Container ID, entry number)` names one Entry, so two renames of it in one
  // record are refused by the same strict order.
  it('refuses a duplicate rename', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'renames'), 1).set('entry', 0n);
    }, renamedRecord());
    expect(errorCode(() => read(payload))).toBe('control_payload_out_of_order');
  });

  // EP-1, EP-2: a rename's path is a stored Entry Path, refused rather than
  // repaired when it is not NFC or not in EP-2's shape.
  it('refuses a rename whose path is not an Entry Path', () => {
    const unnormalized = tampered((map) => {
      mapAt(arrayField(map, 'renames'), 0).set('path', 'albums/cafe\u0301.jpg');
    }, renamedRecord());
    expect(errorCode(() => read(unnormalized))).toBe('unnormalized_entry_path');

    for (const malformed of ['albums/../etc', '/albums/a.jpg', 'albums//a.jpg', '']) {
      const payload = tampered((map) => {
        mapAt(arrayField(map, 'renames'), 0).set('path', malformed);
      }, renamedRecord());
      expect(errorCode(() => read(payload)), JSON.stringify(malformed)).toBe(
        'malformed_entry_path',
      );
    }
  });

  // Derived Entries are not renamed by this version, so no rename gives an Entry
  // a path in their reserved namespace.
  it('refuses a rename into the derived namespace', () => {
    for (const path of ['.coffret-derived/thumb/a.jpg', '.coffret-derived']) {
      const payload = tampered((map) => {
        mapAt(arrayField(map, 'renames'), 2).set('path', path);
      }, renamedRecord());
      expect(errorCode(() => read(payload)), path).toBe('rename_into_reserved_namespace');
    }
  });

  // FM-9: an entry number is a u32.
  it('refuses a rename whose entry number is past 32 bits', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'renames'), 0).set('entry', 0x1_0000_0000n);
    }, renamedRecord());
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  it('refuses a missing field', () => {
    const payload = tampered((map) => map.delete('removals'));
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  it('refuses a field of the wrong shape', () => {
    const payload = tampered((map) => map.set('keyring_set_digest', 9n));
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  // FM-12, KL-3: two spellings of one digest would name one replica set twice.
  it('refuses a Keyring digest that is not lowercase hex', () => {
    const payload = tampered((map) => map.set('keyring_set_digest', 'BEEF'));
    expect(errorCode(() => read(payload))).toBe('invalid_set_digest');
  });

  // KL-2: a set of zero replicas can never be complete, so no commit selects it.
  it('refuses a Keyring replica count of zero', () => {
    const payload = tampered((map) => map.set('keyring_replica_count', 0n));
    expect(errorCode(() => read(payload))).toBe('invalid_replica_count');
  });

  // The same tuple, on the way out: a commitment is a plain interface here, so
  // an encoder that took one on trust would write a record every reader refuses.
  it('refuses to write a Keyring commitment no reader would take', () => {
    const source = record();
    const wrote = (commitment: Partial<JournalRecord['keyring']>) =>
      errorCode(() =>
        encodeJournalRecord({ ...source, keyring: { ...source.keyring, ...commitment } }),
      );
    expect(wrote({ setDigest: 'BEEF' })).toBe('invalid_set_digest');
    expect(wrote({ replicaCount: 0 })).toBe('invalid_replica_count');
  });

  it('refuses a removal that is not sixteen bytes', () => {
    const payload = tampered((map) => {
      const removals = arrayField(map, 'removals');
      removals[0] = new Uint8Array(4).fill(0x11);
      map.set('removals', removals);
    });
    expect(errorCode(() => read(payload))).toBe('invalid_byte_length');
  });

  it('refuses a removal that is not a byte string', () => {
    const payload = tampered((map) => {
      const removals = arrayField(map, 'removals');
      removals[0] = '11111111';
      map.set('removals', removals);
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  // EP-1, FM-9: an Entry's captured path is the name a Container's meta section
  // records, and FM-9 holds that name to EP-1 as well, so a decomposed one is
  // refused the way a decomposed current path is.
  it('refuses a captured path that is not in NFC', () => {
    const payload = tampered((map) => {
      const entries = arrayField(mapAt(arrayField(map, 'additions'), 0), 'entries');
      mapAt(entries, 0).set('captured_path', 'cafe\u0301.txt');
    });
    expect(errorCode(() => read(payload))).toBe('unnormalized_entry_path');
  });

  // FM-15: `captured_path` is written only where it differs from `path`, so
  // one state has one encoding; a map carrying it equal to `path` is a second
  // spelling of a map without it, and is refused.
  it('refuses a captured path equal to the path', () => {
    const payload = tampered((map) => {
      const entries = arrayField(mapAt(arrayField(map, 'additions'), 0), 'entries');
      mapAt(entries, 0).set('captured_path', mapAt(entries, 0).get('path'));
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  // FM-15: `deposited_at` is required in every addition entry, so a map
  // without it is refused rather than read as an Entry with no deposit time.
  it('refuses an addition entry without a deposit time', () => {
    const payload = tampered((map) => {
      const entries = arrayField(mapAt(arrayField(map, 'additions'), 0), 'entries');
      mapAt(entries, 0).delete('deposited_at');
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
    expect(() => read(payload)).toThrow(/deposited_at/);
  });

  // FM-15: each element of an entry table is one entry map — the same values
  // FM-9's is, under the catalog's own keys.
  it('refuses an entry that is not an entry map', () => {
    const payload = tampered((map) => {
      const additions = arrayField(map, 'additions');
      const entries = arrayField(mapAt(additions, 0), 'entries');
      entries[0] = 'not an entry';
      mapAt(additions, 0).set('entries', entries);
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  // FM-9, FM-15, FM-19: an addition carries the entry table of the Container it
  // adds, and each row of it places an Entry inside a plaintext stream whose
  // positions the format bounds. A row whose `offset` and `size` end past the
  // last of them places nothing, so the record does not decode — the same
  // refusal a meta section carrying such a row gets, because it is the same
  // table.
  it('refuses an entry whose extent lies past the end of the address space', () => {
    const payload = tampered((map) => {
      const entries = arrayField(mapAt(arrayField(map, 'additions'), 0), 'entries');
      mapAt(entries, 0).set('offset', MAX_FORMAT_INTEGER);
    });
    expect(errorCode(() => read(payload))).toBe('stream_too_long');
  });

  // FM-19: every unsigned integer a control payload carries is below 2^63,
  // whichever field carries it — so one field's check is every field's, and the
  // detail names the key and the number it found there.
  it('refuses a payload integer past the integer range the format admits', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'additions'), 0).set('ciphertext_len', MAX_FORMAT_INTEGER + 1n);
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');

    const atTheBound = tampered((map) => {
      mapAt(arrayField(map, 'additions'), 0).set('ciphertext_len', MAX_FORMAT_INTEGER);
    });
    expect(read(atTheBound).additions[0].container.ciphertextLength).toBe(MAX_FORMAT_INTEGER);
  });

  // FM-19: the bound holds on the way out too, so a record this package writes
  // is never one its own reader — or the Rust implementation's — refuses. A
  // Container's claimed ciphertext length and an entry row's extent are the
  // numbers a caller hands over, so both are held to it where they are written.
  it('refuses to write an integer past the integer range the format admits', () => {
    const withLength = record();
    withLength.additions[0].container.ciphertextLength = MAX_FORMAT_INTEGER + 1n;
    expect(errorCode(() => encodeJournalRecord(withLength))).toBe('control_payload_encode_failed');

    const withExtent = record();
    withExtent.additions[0].entries[0].size = MAX_FORMAT_INTEGER + 1n;
    expect(errorCode(() => encodeJournalRecord(withExtent))).toBe('control_payload_encode_failed');
  });

  // PK-15: a spelling this format version has no kind for is refused rather
  // than guessed at.
  it('refuses an addition of an unknown kind', () => {
    const payload = tampered((map) => {
      mapAt(arrayField(map, 'additions'), 0).set('kind', 'archive');
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
  });

  // FM-11 takes the padding off before this reader sees a body, so bytes after
  // the map are bytes no writer following the rule left there.
  it('refuses bytes after the body map', () => {
    const payload = encodeJournalRecord(record());
    const body = new Uint8Array(payload.body.length + 1);
    body.set(payload.body, 0);
    expect(errorCode(() => read({ ...payload, body }))).toBe('malformed_journal_record');
  });
});

describe('a Journal record keyed by field name (spec: FM-15)', () => {
  // Every field travels under the field number the registry assigns it, and a
  // text key is no field's spelling, so a map carrying its fields only under
  // their names is missing them — refused by the name of the first one asked
  // for.
  it('is refused as missing its fields, by name', () => {
    const read = (): unknown => decodeJournalRecord(spelledByName(encodeJournalRecord(record()), JOURNAL_RECORD), GENERATION);
    expect(errorCode(read)).toBe('malformed_journal_record');
    expect(read).toThrow(/schema/);
  });
});
