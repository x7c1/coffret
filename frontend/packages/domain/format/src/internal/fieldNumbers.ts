/**
 * The field numbers every stored CBOR map carries its fields under
 * (spec: FM-9, FM-15, FM-16, FM-17).
 *
 * A field's name is how the specification, this package's readers and
 * writers, and every refusal speak of it, and never how it travels: on the wire
 * each field is carried as a CBOR map key equal to the field number the
 * field-number registry, `docs/spec/format/field-numbers.json`, assigns it
 * within its map. The registry is normative and this module is this package's
 * declaration of it — one {@link MapFields} per map kind — which a test holds
 * against the registry map by map, in both directions.
 *
 * The codecs keep reading and writing fields by name. The translation between
 * the two spellings happens here, at the edge where a map becomes bytes or
 * bytes become a map:
 *
 * - {@link toWire} turns a map keyed by names into the map the wire carries. A
 *   name the map kind does not declare, or declares only as retired, is a
 *   writer bug and refused, so no retired field number is ever written.
 * - {@link fromWire} turns a wire map back into one keyed by names. A field
 *   number this build does not know is dropped, as is a retired one, and so is
 *   any text map key: a text key is no field's spelling, so a map carrying its
 *   required fields only under text keys reaches the reader without them and is
 *   refused as missing.
 * - {@link namedView} is the translation for a person reading a payload:
 *   nothing is dropped, so a map key the registry does not name stays visible.
 */

import { fail, type CoffretErrorCode } from '../errors.js';

/** One row of the field-number registry: a field of a map kind and its field number. */
export interface RegisteredField {
  readonly name: string;
  readonly number: number;
  /** A retired field number is never read or written again, and never reassigned. */
  readonly retired?: true;
  /** The map kind this field's value is, or the kind of every map in its array. */
  readonly holds?: () => MapFields;
}

/** The fields of one map kind, with the field numbers the registry assigns them. */
export interface MapFields {
  /** The map kind's name, as the registry's `map` column spells it. */
  readonly map: string;
  readonly fields: readonly RegisteredField[];
}

/** The field number `master_key_epoch` travels under in every control payload map. */
export const MASTER_KEY_EPOCH_FIELD = 0;

function field(name: string, number: number, holds?: () => MapFields): RegisteredField {
  return holds === undefined ? { name, number } : { name, number, holds };
}

/** The parent reference an entry map carries, in whichever map (spec: FM-9). */
export const DERIVED_FROM: MapFields = {
  map: 'derived_from',
  fields: [field('container_id', 0), field('captured_path', 1)],
};

/** One row of a meta section's entry table (spec: FM-9). */
export const META_ENTRY: MapFields = {
  map: 'meta_entry',
  fields: [
    field('captured_path', 0),
    field('offset', 1),
    field('size', 2),
    field('captured_mtime', 3),
    field('hash', 4),
    field('captured_btime', 5),
    field('mime', 6),
    field('derived_from', 7, () => DERIVED_FROM),
  ],
};

/** The meta section's own map (spec: FM-9). */
export const META: MapFields = {
  map: 'meta',
  fields: [
    field('schema', 1),
    field('kind', 2),
    field('pad_len', 3),
    field('entries', 4, () => META_ENTRY),
  ],
};

/** One element of an addition's entry table (spec: FM-15). */
export const ADDITION_ENTRY: MapFields = {
  map: 'addition_entry',
  fields: [
    field('path', 0),
    field('offset', 1),
    field('size', 2),
    field('mtime', 3),
    field('hash', 4),
    field('btime', 5),
    field('mime', 6),
    field('derived_from', 7, () => DERIVED_FROM),
    field('captured_path', 8),
    field('deposited_at', 9),
  ],
};

/** One element of a Journal record's `additions` (spec: FM-15). */
export const ADDITION: MapFields = {
  map: 'addition',
  fields: [
    field('id', 0),
    field('kind', 1),
    field('ciphertext_hash', 2),
    field('ciphertext_len', 3),
    field('object_ref', 4),
    field('entries', 5, () => ADDITION_ENTRY),
  ],
};

/** A Journal record's payload map (spec: FM-15). */
export const JOURNAL_RECORD: MapFields = {
  map: 'journal_record',
  fields: [
    field('master_key_epoch', MASTER_KEY_EPOCH_FIELD),
    field('schema', 1),
    field('prev', 2),
    field('next_commit_slot', 3),
    field('snapshot_slot', 4),
    field('keyring_generation', 5),
    field('keyring_replica_count', 6),
    field('keyring_set_digest', 7),
    field('additions', 8, () => ADDITION),
    field('removals', 9),
    field('renames', 10, () => RENAME),
    field('required_schema', 11),
    field('library_name', 12),
  ],
};

/** One element of a Journal record's `renames` (spec: FM-15). */
export const RENAME: MapFields = {
  map: 'rename',
  fields: [field('container', 0), field('entry', 1), field('path', 2)],
};

/** One element of an Index Snapshot's `containers` (spec: FM-16). */
export const SNAPSHOT_CONTAINER: MapFields = {
  map: 'snapshot_container',
  fields: [
    field('id', 0),
    field('kind', 1),
    field('ciphertext_hash', 2),
    field('ciphertext_len', 3),
    field('object_ref', 4),
  ],
};

/** One element of an Index Snapshot's `entries` (spec: FM-16). */
export const SNAPSHOT_ENTRY: MapFields = {
  map: 'snapshot_entry',
  fields: [
    field('path', 0),
    field('offset', 1),
    field('size', 2),
    field('mtime', 3),
    field('hash', 4),
    field('btime', 5),
    field('mime', 6),
    field('derived_from', 7, () => DERIVED_FROM),
    field('captured_path', 8),
    field('container', 9),
    field('entry', 10),
    field('deposited_at', 11),
  ],
};

/** An Index Snapshot's payload map, ordinary and activation alike (spec: FM-16). */
export const INDEX_SNAPSHOT: MapFields = {
  map: 'index_snapshot',
  fields: [
    field('master_key_epoch', MASTER_KEY_EPOCH_FIELD),
    field('schema', 1),
    field('head_generation', 2),
    field('next_commit_slot', 3),
    field('journal_generation', 4),
    field('keyring_generation', 5),
    field('keyring_replica_count', 6),
    field('keyring_set_digest', 7),
    field('containers', 8, () => SNAPSHOT_CONTAINER),
    field('entries', 9, () => SNAPSHOT_ENTRY),
    field('base_head_generation', 10),
    field('activation_slot', 11),
    field('library_name', 12),
  ],
};

/** One element of a Keyring's `mapping` (spec: FM-17). */
export const KEYRING_ELEMENT: MapFields = {
  map: 'keyring_element',
  fields: [field('id', 0), field('envelope', 1), field('key_lost', 2)],
};

/** A Keyring replica's payload map (spec: FM-17). */
export const KEYRING: MapFields = {
  map: 'keyring',
  fields: [
    field('master_key_epoch', MASTER_KEY_EPOCH_FIELD),
    field('schema', 1),
    field('mapping', 2, () => KEYRING_ELEMENT),
  ],
};

/** Every map kind this package declares, which is every one the registry lists. */
export const ALL_MAP_FIELDS: readonly MapFields[] = [
  META,
  META_ENTRY,
  DERIVED_FROM,
  JOURNAL_RECORD,
  RENAME,
  ADDITION,
  ADDITION_ENTRY,
  INDEX_SNAPSHOT,
  SNAPSHOT_CONTAINER,
  SNAPSHOT_ENTRY,
  KEYRING,
  KEYRING_ELEMENT,
];

function liveByName(kind: MapFields, name: string): RegisteredField | undefined {
  return kind.fields.find((field) => field.retired !== true && field.name === name);
}

function liveByNumber(kind: MapFields, number: number): RegisteredField | undefined {
  return kind.fields.find((field) => field.retired !== true && field.number === number);
}

/** A CBOR map key as the field number it carries, if it is an unsigned integer. */
export function fieldNumber(key: unknown): number | undefined {
  if (typeof key === 'number' && Number.isSafeInteger(key) && key >= 0) {
    return key;
  }
  if (typeof key === 'bigint' && key >= 0n && key <= BigInt(Number.MAX_SAFE_INTEGER)) {
    return Number(key);
  }
  return undefined;
}

/**
 * A value keyed by field names, as the wire carries it: every map of `kind` —
 * `value` itself, or each map in it where it is an array — keyed by field
 * numbers, and every map nested in one by its own kind's.
 */
export function toWire(value: unknown, kind: MapFields, code: CoffretErrorCode): unknown {
  if (value instanceof Map) {
    const wire = new Map<number, unknown>();
    for (const [name, inner] of value) {
      const field = typeof name === 'string' ? liveByName(kind, name) : undefined;
      if (field === undefined) {
        fail(code, `the ${kind.map} map has no field number for the field ${String(name)}`);
      }
      wire.set(field.number, field.holds === undefined ? inner : toWire(inner, field.holds(), code));
    }
    return wire;
  }
  if (Array.isArray(value)) {
    return value.map((item) => toWire(item, kind, code));
  }
  return value;
}

/**
 * A value read off the wire, keyed by field names for a reader: every field
 * number this build knows becomes its field's name, and every other map key —
 * an unknown field number, a retired one, any text — is dropped (spec: FM-9).
 */
export function fromWire(value: unknown, kind: MapFields): unknown {
  if (value instanceof Map) {
    const named = new Map<string, unknown>();
    for (const [key, inner] of value) {
      const number = fieldNumber(key);
      const field = number === undefined ? undefined : liveByNumber(kind, number);
      if (field !== undefined) {
        named.set(field.name, field.holds === undefined ? inner : fromWire(inner, field.holds()));
      }
    }
    return named;
  }
  if (Array.isArray(value)) {
    return value.map((item) => fromWire(item, kind));
  }
  return value;
}

/**
 * A value read off the wire, keyed by field names for a person reading it.
 *
 * Nothing is dropped: a field number the registry names for this map becomes
 * that name, one it does not name stays the integer, and a text map key — which
 * is no field's spelling — is shown quoted, so it cannot pass for the field it
 * spells.
 */
export function namedView(value: unknown, kind: MapFields): unknown {
  if (value instanceof Map) {
    const named = new Map<unknown, unknown>();
    for (const [key, inner] of value) {
      const number = fieldNumber(key);
      const field = number === undefined ? undefined : liveByNumber(kind, number);
      if (field !== undefined) {
        named.set(field.name, field.holds === undefined ? inner : namedView(inner, field.holds()));
      } else {
        named.set(typeof key === 'string' ? JSON.stringify(key) : key, inner);
      }
    }
    return named;
  }
  if (Array.isArray(value)) {
    return value.map((item) => namedView(item, kind));
  }
  return value;
}
