import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { Generation } from '../model/generation.js';
import type { JournalRecord } from '../model/journalRecord.js';
import { decodeJournalRecord, encodeJournalRecord } from './journalRecord.js';
import type { ControlPayload } from './payload.js';
import {
  EPOCH,
  GENERATION,
  addition,
  arrayField,
  bodyMap,
  containerId,
  firstRecord,
  mapAt,
  record,
  withBodyMap,
} from './payloadSchemas.testing.js';

/** The record as the encoder puts it on the wire: arrays in Container ID order. */
function canonical(source: JournalRecord): JournalRecord {
  return {
    ...source,
    additions: [...source.additions].sort((left, right) =>
      left.container.id.toHex() < right.container.id.toHex() ? -1 : 1,
    ),
    removals: [...source.removals].sort((left, right) => (left.toHex() < right.toHex() ? -1 : 1)),
  };
}

/** A record payload with one field changed by hand, as a reader meets it. */
function tampered(change: (map: Map<unknown, unknown>) => void): ControlPayload {
  const payload = encodeJournalRecord(record());
  const map = bodyMap(payload);
  change(map);
  return withBodyMap(payload.masterKeyEpoch, map);
}

function read(payload: ControlPayload): JournalRecord {
  return decodeJournalRecord(payload, GENERATION);
}

describe('Journal record payload (FM-15)', () => {
  // The Keyring tuple it commits to, both slots it reserves, the Containers it
  // added with their entry tables, and the ones it removed all come back as
  // they went in.
  it('round-trips a record with everything', () => {
    const source = record();
    expect(read(encodeJournalRecord(source))).toEqual(canonical(source));
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
    const map = bodyMap(encodeJournalRecord(firstRecord()));
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
    const map = bodyMap(encoded);
    map.set('prev', 0n);
    const payload = withBodyMap(encoded.masterKeyEpoch, map);
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
    const map = bodyMap(payload);
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
    const removals = arrayField(bodyMap(encodeJournalRecord(record())), 'removals');
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
      map.set('future_field', 'whatever');
      for (const item of arrayField(map, 'additions')) {
        (item as Map<unknown, unknown>).set('future_addition_field', 1n);
      }
      map.set('schema', 2n);
    });
    expect(read(payload)).toEqual(canonical(record()));
  });
});

describe('Journal record payloads a reader refuses (FM-15)', () => {
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

  // FM-9's rule for the meta section, applied here.
  it('refuses a schema below one', () => {
    const payload = tampered((map) => map.set('schema', 0n));
    expect(errorCode(() => read(payload))).toBe('unsupported_journal_record_schema');
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

  // FM-9: each element of an entry table is exactly FM-9's entry map, the same
  // reading the meta section gets.
  it("refuses an entry that is not FM-9's entry map", () => {
    const payload = tampered((map) => {
      const additions = arrayField(map, 'additions');
      const entries = arrayField(mapAt(additions, 0), 'entries');
      entries[0] = 'not an entry';
      mapAt(additions, 0).set('entries', entries);
    });
    expect(errorCode(() => read(payload))).toBe('malformed_journal_record');
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
