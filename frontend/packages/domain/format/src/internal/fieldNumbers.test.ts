import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  ALL_MAP_FIELDS,
  INDEX_SNAPSHOT,
  JOURNAL_RECORD,
  KEYRING,
  MASTER_KEY_EPOCH_FIELD,
  fromWire,
  namedView,
  toWire,
  type MapFields,
} from './fieldNumbers.js';

/** The field-number registry the declarations are held against (spec: FM-9). */
const REGISTRY = new URL('../../../../../../docs/spec/format/field-numbers.json', import.meta.url);

interface Row {
  map: string;
  name: string;
  number: number;
  retired?: boolean;
}

function rowsOf(rows: readonly Row[]): Map<string, Set<string>> {
  const maps = new Map<string, Set<string>>();
  for (const row of rows) {
    const rendered = `${row.name}=${row.number}${row.retired === true ? ' (retired)' : ''}`;
    const set = maps.get(row.map) ?? new Set<string>();
    set.add(rendered);
    maps.set(row.map, set);
  }
  return maps;
}

function registry(): Map<string, Set<string>> {
  const json = JSON.parse(readFileSync(REGISTRY, 'utf8')) as { fields: Row[] };
  return rowsOf(json.fields);
}

function declared(): Map<string, Set<string>> {
  return rowsOf(
    ALL_MAP_FIELDS.flatMap((kind) =>
      kind.fields.map((field) => ({
        map: kind.map,
        name: field.name,
        number: field.number,
        ...(field.retired === true ? { retired: true } : {}),
      })),
    ),
  );
}

describe('the field-number registry', () => {
  it('declares every map this package declares, and no other', () => {
    expect([...declared().keys()].sort()).toEqual([...registry().keys()].sort());
  });

  it('matches this package map by map, in both directions', () => {
    const ours = declared();
    for (const [map, rows] of registry()) {
      const mine = ours.get(map) ?? new Set<string>();
      expect([...rows].filter((row) => !mine.has(row)), `${map}: missing here`).toEqual([]);
      expect([...mine].filter((row) => !rows.has(row)), `${map}: extra here`).toEqual([]);
    }
  });

  it('uses no field number twice within a map, and no live name twice', () => {
    for (const kind of ALL_MAP_FIELDS) {
      const numbers = kind.fields.map((field) => field.number);
      expect(new Set(numbers).size, kind.map).toBe(numbers.length);
      const live = kind.fields.filter((field) => field.retired !== true).map((field) => field.name);
      expect(new Set(live).size, kind.map).toBe(live.length);
    }
  });

  it('nests only map kinds it declares', () => {
    for (const kind of ALL_MAP_FIELDS) {
      for (const field of kind.fields) {
        if (field.holds !== undefined) {
          expect(ALL_MAP_FIELDS, `${kind.map}.${field.name}`).toContain(field.holds());
        }
      }
    }
  });

  // FM-13: the framing finds the epoch before it knows whose map it holds.
  it("gives the epoch the framing's field number in every payload map", () => {
    for (const kind of [JOURNAL_RECORD, INDEX_SNAPSHOT, KEYRING]) {
      const epoch = kind.fields.find((field) => field.name === 'master_key_epoch');
      expect(epoch?.number, kind.map).toBe(MASTER_KEY_EPOCH_FIELD);
    }
  });
});

/** A map kind with one retired field and the live one that replaced it. */
const WITH_RETIRED: MapFields = {
  map: 'with_retired',
  fields: [
    { name: 'kept', number: 0 },
    { name: 'replaced', number: 1, retired: true },
    { name: 'replaced', number: 2 },
  ],
};

describe('the translation', () => {
  it('never reads a retired field number', () => {
    const read = fromWire(
      new Map([
        [0, 10],
        [1, 11],
      ]),
      WITH_RETIRED,
    );
    expect(read).toEqual(new Map([['kept', 10]]));
  });

  it('never writes a retired field number', () => {
    const written = toWire(
      new Map([
        ['kept', 10],
        ['replaced', 12],
      ]),
      WITH_RETIRED,
      'control_payload_encode_failed',
    );
    expect(written).toEqual(
      new Map([
        [0, 10],
        [2, 12],
      ]),
    );
  });

  // FM-9: the maps are forward-open.
  it('steps over an unknown field number', () => {
    const read = fromWire(
      new Map<unknown, unknown>([
        [0, 10],
        [23, 1],
        [1000n, 2],
      ]),
      WITH_RETIRED,
    );
    expect(read).toEqual(new Map([['kept', 10]]));
  });

  it('takes a text key for no field', () => {
    expect(fromWire(new Map([['kept', 10]]), WITH_RETIRED)).toEqual(new Map());
  });

  it('refuses to write a name the map does not declare', () => {
    expect(() =>
      toWire(new Map([['unheard_of', 1]]), WITH_RETIRED, 'control_payload_encode_failed'),
    ).toThrow(/with_retired.*unheard_of/);
  });

  it('keeps in the named view what it cannot name', () => {
    const view = namedView(
      new Map<unknown, unknown>([
        [0, 10],
        [1, 11],
        ['kept', 13],
      ]),
      WITH_RETIRED,
    );
    expect(view).toEqual(
      new Map<unknown, unknown>([
        ['kept', 10],
        [1, 11],
        ['"kept"', 13],
      ]),
    );
  });
});
