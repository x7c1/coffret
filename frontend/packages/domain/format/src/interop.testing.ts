/**
 * The fixture set two implementations exchange, as this side reads and writes
 * it.
 *
 * A fixture set is a directory of opaque byte strings plus a `manifest.json`
 * stating, for every one of them, the key material needed to open it and the
 * values it must decode to:
 *
 * ```text
 * <dir>/manifest.json      what the set states about itself
 * <dir>/objects/<name>     Storage Objects, under the names they are stored as
 * <dir>/blobs/<name>       byte strings that are not Storage Objects
 * ```
 *
 * The manifest states inputs and expectations only — never a length, an offset,
 * a hash, or any other value the format derives — because an expectation the
 * manifest computed itself would prove only that the manifest writer and the
 * reader share a bug. Its field names are snake_case on the wire and camelCase
 * here; the parse below is the one place the two spellings meet, and it is also
 * where a manifest that is not this schema is rejected.
 *
 * This module reads and writes files, which the package itself never does. It
 * is a `.testing.ts` file for that reason: it is excluded from the build and
 * reaches nobody through the package's public surface.
 */

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { decodeCborExact } from './internal/cbor.js';
import { fromHex, toHex } from './internal/bytes.js';
import { CONTAINER_KEY_LENGTH, ContainerKey } from './model/containerKey.js';
import { ContainerId } from './model/containerId.js';
import { Generation } from './model/generation.js';
import { MASTER_KEY_LENGTH, MasterKey } from './model/masterKey.js';
import { MasterKeyEpoch } from './model/masterKeyEpoch.js';
import { ReplicaPosition } from './model/replicaPosition.js';
import {
  CONTAINER_KINDS,
  CONTROL_OBJECT_KINDS,
  type ContainerKind,
  type ControlObjectKind,
} from './model/kinds.js';
import type { Argon2Params } from './storedMasterKey/argon2Params.js';
import type { DerivedFrom } from './model/entry.js';

/** The manifest schema both implementations write and read. */
export const MANIFEST_SCHEMA = 1;

/** The file a fixture set describes itself in. */
export const MANIFEST_FILE = 'manifest.json';

/** Where Storage Objects live inside a fixture set. */
export const OBJECTS_DIR = 'objects';

/** Where byte strings that never reach Storage live inside a fixture set. */
export const BLOBS_DIR = 'blobs';

/**
 * The Container fixtures every set carries, whichever side wrote it.
 *
 * One of the Packs holds a single Entry, so a kind guessed from the Entry count
 * rather than read from the object (PK-15) fails the exchange.
 */
export const REQUIRED_CONTAINERS = ['one-file', 'multi-entry', 'singleton-pack', 'empty-entries'];

/**
 * The control-object fixtures every set carries — one of each kind (FM-11).
 *
 * The Journal record and the activation Snapshot are both stored under a `head-`
 * name (FM-12), so a set that carries both is a set no implementation can pass
 * by reading a kind off a name.
 */
export const REQUIRED_CONTROL_OBJECTS = [
  'journal',
  'activation-snapshot',
  'keyring-replica',
  'index-snapshot',
];

/** The Key Envelope fixtures every set carries. */
export const REQUIRED_KEY_ENVELOPES = ['key-envelope'];

/** The stored Master Key fixtures every set carries. */
export const REQUIRED_STORED_MASTER_KEYS = ['stored-master-key'];

/** Everything a fixture set states about itself. */
export interface Manifest {
  /** Which implementation wrote the set, for the message on a failure. */
  producer: string;
  /** The Master Key every purpose key in the set derives from. */
  masterKey: MasterKey;
  /** The Passphrase the stored Master Key forms are protected under. */
  passphrase: string;
  /** The Containers in the set. */
  containers: ContainerFixture[];
  /** The control objects in the set. */
  controlObjects: ControlObjectFixture[];
  /** The Key Envelopes in the set. */
  keyEnvelopes: KeyEnvelopeFixture[];
  /** The stored Master Key forms in the set. */
  storedMasterKeys: StoredMasterKeyFixture[];
}

/** One Container in a fixture set, with everything needed to open and check it. */
export interface ContainerFixture {
  /** The name this fixture is known by across both implementations. */
  fixture: string;
  /** Where the bytes live, relative to the fixture directory. */
  file: string;
  /** The name the object is stored under. */
  objectName: string;
  /** The Container ID. */
  containerId: ContainerId;
  /** The key the object is encrypted with. */
  containerKey: ContainerKey;
  /** Whether this Container is one-file or a Pack. */
  kind: ContainerKind;
  /** The chunk size the object was written with. */
  chunkSize: number;
  /** The entries the object must decode to, in plaintext stream order. */
  entries: EntryFixture[];
}

/** One Entry a Container must decode to. */
export interface EntryFixture {
  /** The Library position this Entry occupies. */
  path: string;
  /** The modification time, as whole seconds from the Unix epoch. */
  mtimeSeconds: bigint;
  /** The Entry's plaintext. */
  content: Uint8Array;
  /** Set when this Entry holds data derived from another Entry. */
  derivedFrom?: DerivedFrom;
  /** The media type of the content, when the writer recorded one. */
  mime?: string;
}

/** One control object in a fixture set, with what it must decode to. */
export interface ControlObjectFixture {
  /** The name this fixture is known by across both implementations. */
  fixture: string;
  /** Where the bytes live, relative to the fixture directory. */
  file: string;
  /** The name the object is stored under, in one of FM-12's forms. */
  objectName: string;
  /** Which kind of control state the object carries. */
  kind: ControlObjectKind;
  /**
   * Where the object sits in the Library's control history; the numbering
   * never restarts at a rotation (FM-13).
   */
  generation: Generation;
  /** Which replica this is, out of how many. */
  replica: ReplicaPosition;
  /** The Master Key epoch that encrypted the payload. */
  masterKeyEpoch: MasterKeyEpoch;
  /** The kind's own payload fields. */
  body: BodyField[];
}

/** One Key Envelope in a fixture set, and the key it must unwrap to. */
export interface KeyEnvelopeFixture {
  /** The name this fixture is known by across both implementations. */
  fixture: string;
  /** Where the 72 bytes live, relative to the fixture directory. */
  file: string;
  /** The Container the envelope is bound to. */
  containerId: ContainerId;
  /** The Container Key the envelope must unwrap to. */
  containerKey: ContainerKey;
}

/** One stored Master Key form, and what unlocking it must give. */
export interface StoredMasterKeyFixture {
  /** The name this fixture is known by across both implementations. */
  fixture: string;
  /** Where the bytes live, relative to the fixture directory. */
  file: string;
  /** The Master Key unlocking must yield. */
  masterKey: MasterKey;
  /** The epoch that key belongs to. */
  epoch: MasterKeyEpoch;
  /** The Argon2id cost the form was written at. */
  argon2: Argon2Params;
}

/**
 * One value a manifest may state for a payload body field.
 *
 * A body is the kind's own CBOR map, which the framing treats as opaque. The
 * manifest therefore describes it field by field in a small typed vocabulary
 * rather than as bytes: the two implementations legitimately order and spell map
 * entries differently, so only the decoded fields can be compared. `array` and
 * `map` are what let it describe the payloads whose fields are not flat — a
 * Journal record's additions each carry an entry table (FM-15), an Index
 * Snapshot's Containers and Entries are arrays of maps (FM-16), and a Keyring's
 * mapping is an array of maps too (FM-17). `bool` is there for the one field
 * that is one: a Keyring's `key_lost` marker.
 */
export type BodyValue =
  | { type: 'uint'; value: bigint }
  | { type: 'int'; value: bigint }
  | { type: 'bool'; value: boolean }
  | { type: 'text'; value: string }
  | { type: 'bytes'; value: Uint8Array }
  | { type: 'array'; value: BodyValue[] }
  | { type: 'map'; value: BodyField[] };

/** One field of a control object's payload body, as the manifest states it. */
export type BodyField = BodyValue & { key: string };

/**
 * Reads a decoded payload body back into fields, sorted by key at every level.
 *
 * Sorting is what lets a comparison ignore map order, which is a serializer's
 * choice and not part of the format. Array order is left alone, because the
 * order of every array in a payload is part of what its rule states (FM-15,
 * FM-16).
 */
export function decodeBodyFields(body: Uint8Array): BodyField[] {
  const map = decodeCborExact(body, 'malformed_control_payload');
  if (!(map instanceof Map)) {
    throw new Error('a control-object payload body is a CBOR map');
  }
  return fieldsOf(map);
}

/** Orders fields by key at every level, so two bodies compare as maps. */
export function sortBodyFields(fields: readonly BodyField[]): BodyField[] {
  return [...fields]
    .map((field) => ({ ...field, ...sortBodyValue(field) }) as BodyField)
    .sort((left, right) => compareText(left.key, right.key));
}

function sortBodyValue(value: BodyValue): BodyValue {
  switch (value.type) {
    case 'array':
      return { type: 'array', value: value.value.map(sortBodyValue) };
    case 'map':
      return { type: 'map', value: sortBodyFields(value.value) };
    default:
      return value;
  }
}

function fieldsOf(map: Map<unknown, unknown>): BodyField[] {
  return sortBodyFields(
    [...map].map(([key, value]) => {
      if (typeof key !== 'string') {
        throw new Error('a payload body key is text');
      }
      return { key, ...bodyValue(value, key) } as BodyField;
    }),
  );
}

function bodyValue(value: unknown, key: string): BodyValue {
  const integer =
    typeof value === 'bigint'
      ? value
      : typeof value === 'number' && Number.isSafeInteger(value)
        ? BigInt(value)
        : undefined;
  if (integer !== undefined) {
    // A number at or above zero has one spelling, whichever side stated it:
    // reading a payload back cannot tell an unsigned zero from a signed one.
    return integer < 0n ? { type: 'int', value: integer } : { type: 'uint', value: integer };
  }
  if (typeof value === 'boolean') {
    return { type: 'bool', value };
  }
  if (typeof value === 'string') {
    return { type: 'text', value };
  }
  if (value instanceof Uint8Array) {
    return { type: 'bytes', value };
  }
  if (Array.isArray(value)) {
    return { type: 'array', value: value.map((item) => bodyValue(item, key)) };
  }
  if (value instanceof Map) {
    return { type: 'map', value: fieldsOf(value) };
  }
  throw new Error(`payload body field ${JSON.stringify(key)} is not a type this exchange states`);
}

function compareText(left: string, right: string): number {
  if (left === right) {
    return 0;
  }
  return left < right ? -1 : 1;
}

/** A fixture set being read. */
export class FixtureReader {
  /** What the set states about itself. */
  readonly manifest: Manifest;
  readonly #root: string;

  constructor(root: string) {
    this.#root = root;
    this.manifest = parseManifest(readFileSync(join(root, MANIFEST_FILE), 'utf8'));
  }

  /**
   * Reads one file the manifest points at.
   *
   * The path is taken apart and rebuilt rather than joined as given, so a
   * manifest cannot send a reader outside the set it describes.
   */
  read(relative: string): Uint8Array {
    let path = this.#root;
    for (const segment of relative.split('/')) {
      if (segment === '' || segment === '.' || segment === '..') {
        throw new Error(`${JSON.stringify(relative)} is not a path inside the fixture set`);
      }
      path = join(path, segment);
    }
    return Uint8Array.from(readFileSync(path));
  }
}

/** A fixture set being written. */
export class FixtureWriter {
  readonly #root: string;

  constructor(root: string) {
    this.#root = root;
    for (const directory of [OBJECTS_DIR, BLOBS_DIR]) {
      mkdirSync(join(root, directory), { recursive: true });
    }
  }

  /** Writes one file, returning the manifest-relative path to it. */
  write(directory: string, name: string, bytes: Uint8Array): string {
    writeFileSync(join(this.#root, directory, name), bytes);
    return `${directory}/${name}`;
  }

  /** Writes the manifest, which is the last thing a complete set gains. */
  writeManifest(manifest: Manifest): void {
    writeFileSync(join(this.#root, MANIFEST_FILE), `${renderManifest(manifest)}\n`);
  }
}

/** Reads a manifest, rejecting anything that is not this schema. */
export function parseManifest(text: string): Manifest {
  const root = readObject(JSON.parse(text), MANIFEST_FILE);
  const schema = readInteger(root, 'schema');
  if (schema !== MANIFEST_SCHEMA) {
    throw new Error(
      `manifest schema ${schema} is not the schema ${MANIFEST_SCHEMA} this build reads`,
    );
  }
  return {
    producer: readText(root, 'producer'),
    masterKey: readMasterKey(root, 'master_key'),
    passphrase: readText(root, 'passphrase'),
    containers: readArray(root, 'containers').map(parseContainer),
    controlObjects: readArray(root, 'control_objects').map(parseControlObject),
    keyEnvelopes: readArray(root, 'key_envelopes').map(parseKeyEnvelope),
    storedMasterKeys: readArray(root, 'stored_master_keys').map(parseStoredMasterKey),
  };
}

/** Spells a manifest as the JSON both implementations read. */
export function renderManifest(manifest: Manifest): string {
  return JSON.stringify(
    {
      schema: MANIFEST_SCHEMA,
      producer: manifest.producer,
      master_key: toHex(manifest.masterKey.bytes()),
      passphrase: manifest.passphrase,
      containers: manifest.containers.map(renderContainer),
      control_objects: manifest.controlObjects.map(renderControlObject),
      key_envelopes: manifest.keyEnvelopes.map(renderKeyEnvelope),
      stored_master_keys: manifest.storedMasterKeys.map(renderStoredMasterKey),
    },
    undefined,
    2,
  );
}

function parseContainer(value: JsonObject): ContainerFixture {
  return {
    fixture: readText(value, 'fixture'),
    file: readText(value, 'file'),
    objectName: readText(value, 'object_name'),
    containerId: ContainerId.fromHex(readText(value, 'container_id')),
    containerKey: ContainerKey.fromBytes(
      fromHex(readText(value, 'container_key'), CONTAINER_KEY_LENGTH * 2),
    ),
    kind: readOneOf(readText(value, 'kind'), CONTAINER_KINDS, 'kind'),
    chunkSize: readInteger(value, 'chunk_size'),
    entries: readArray(value, 'entries').map(parseEntry),
  };
}

function renderContainer(fixture: ContainerFixture): unknown {
  return {
    fixture: fixture.fixture,
    file: fixture.file,
    object_name: fixture.objectName,
    container_id: fixture.containerId.toHex(),
    container_key: toHex(fixture.containerKey.bytes()),
    kind: fixture.kind,
    chunk_size: fixture.chunkSize,
    entries: fixture.entries.map(renderEntry),
  };
}

function parseEntry(value: JsonObject): EntryFixture {
  const entry: EntryFixture = {
    path: readText(value, 'path'),
    mtimeSeconds: BigInt(readInteger(value, 'mtime')),
    content: readHexBytes(value, 'content'),
  };
  const derivedFrom = readOptionalObject(value, 'derived_from');
  if (derivedFrom !== undefined) {
    entry.derivedFrom = {
      containerId: ContainerId.fromHex(readText(derivedFrom, 'container_id')),
      path: readText(derivedFrom, 'path'),
    };
  }
  const mime = readOptionalText(value, 'mime');
  if (mime !== undefined) {
    entry.mime = mime;
  }
  return entry;
}

function renderEntry(entry: EntryFixture): unknown {
  return {
    path: entry.path,
    mtime: safeInteger(entry.mtimeSeconds, 'mtime'),
    content: toHex(entry.content),
    ...(entry.derivedFrom === undefined
      ? {}
      : {
          derived_from: {
            container_id: entry.derivedFrom.containerId.toHex(),
            path: entry.derivedFrom.path,
          },
        }),
    ...(entry.mime === undefined ? {} : { mime: entry.mime }),
  };
}

function parseControlObject(value: JsonObject): ControlObjectFixture {
  return {
    fixture: readText(value, 'fixture'),
    file: readText(value, 'file'),
    objectName: readText(value, 'object_name'),
    kind: readOneOf(readText(value, 'kind'), CONTROL_OBJECT_KINDS, 'kind'),
    generation: Generation.of(BigInt(readInteger(value, 'generation'))),
    replica: ReplicaPosition.of(
      readInteger(value, 'replica_index'),
      readInteger(value, 'replica_count'),
    ),
    masterKeyEpoch: MasterKeyEpoch.of(BigInt(readInteger(value, 'master_key_epoch'))),
    body: readArray(value, 'body').map(parseBodyField),
  };
}

function renderControlObject(fixture: ControlObjectFixture): unknown {
  return {
    fixture: fixture.fixture,
    file: fixture.file,
    object_name: fixture.objectName,
    kind: fixture.kind,
    generation: safeInteger(fixture.generation.value, 'generation'),
    replica_index: fixture.replica.index,
    replica_count: fixture.replica.count,
    master_key_epoch: safeInteger(fixture.masterKeyEpoch.value, 'master_key_epoch'),
    body: fixture.body.map(renderBodyField),
  };
}

function parseBodyField(value: JsonObject): BodyField {
  const key = readText(value, 'key');
  return { key, ...parseBodyValue(value, key) } as BodyField;
}

function parseBodyValue(value: JsonObject, key: string): BodyValue {
  const type = readText(value, 'type');
  switch (type) {
    case 'uint':
      return { type, value: BigInt(readInteger(value, 'value')) };
    case 'int': {
      // Mirrors the writer's rule above: a value at or above zero is an
      // unsigned one.
      const number = BigInt(readInteger(value, 'value'));
      return number < 0n ? { type, value: number } : { type: 'uint', value: number };
    }
    case 'bool':
      return { type, value: readBoolean(value, 'value') };
    case 'text':
      return { type, value: readText(value, 'value') };
    case 'bytes':
      return { type, value: readHexBytes(value, 'value') };
    case 'array':
      return {
        type,
        value: readArray(value, 'value').map((item, index) => parseBodyValue(item, `${key}[${index}]`)),
      };
    case 'map':
      return { type, value: readArray(value, 'value').map(parseBodyField) };
    default:
      throw new Error(
        `payload body field ${JSON.stringify(key)} has unknown type ${JSON.stringify(type)}`,
      );
  }
}

function renderBodyField(field: BodyField): unknown {
  return { key: field.key, ...(renderBodyValue(field, field.key) as object) };
}

function renderBodyValue(value: BodyValue, key: string): unknown {
  switch (value.type) {
    case 'uint':
    case 'int':
      return { type: value.type, value: safeInteger(value.value, key) };
    case 'bool':
    case 'text':
      return { type: value.type, value: value.value };
    case 'bytes':
      return { type: value.type, value: toHex(value.value) };
    case 'array':
      return {
        type: value.type,
        value: value.value.map((item, index) => renderBodyValue(item, `${key}[${index}]`)),
      };
    case 'map':
      return { type: value.type, value: value.value.map(renderBodyField) };
  }
}

function parseKeyEnvelope(value: JsonObject): KeyEnvelopeFixture {
  return {
    fixture: readText(value, 'fixture'),
    file: readText(value, 'file'),
    containerId: ContainerId.fromHex(readText(value, 'container_id')),
    containerKey: ContainerKey.fromBytes(
      fromHex(readText(value, 'container_key'), CONTAINER_KEY_LENGTH * 2),
    ),
  };
}

function renderKeyEnvelope(fixture: KeyEnvelopeFixture): unknown {
  return {
    fixture: fixture.fixture,
    file: fixture.file,
    container_id: fixture.containerId.toHex(),
    container_key: toHex(fixture.containerKey.bytes()),
  };
}

function parseStoredMasterKey(value: JsonObject): StoredMasterKeyFixture {
  const argon2 = readObjectField(value, 'argon2');
  return {
    fixture: readText(value, 'fixture'),
    file: readText(value, 'file'),
    masterKey: readMasterKey(value, 'master_key'),
    epoch: MasterKeyEpoch.of(BigInt(readInteger(value, 'epoch'))),
    argon2: {
      memoryKib: readInteger(argon2, 'memory_kib'),
      iterations: readInteger(argon2, 'iterations'),
      parallelism: readInteger(argon2, 'parallelism'),
    },
  };
}

function renderStoredMasterKey(fixture: StoredMasterKeyFixture): unknown {
  return {
    fixture: fixture.fixture,
    file: fixture.file,
    master_key: toHex(fixture.masterKey.bytes()),
    epoch: safeInteger(fixture.epoch.value, 'epoch'),
    argon2: {
      memory_kib: fixture.argon2.memoryKib,
      iterations: fixture.argon2.iterations,
      parallelism: fixture.argon2.parallelism,
    },
  };
}

/** A JSON object the manifest reader is walking, remembered by its path. */
interface JsonObject {
  fields: Record<string, unknown>;
  path: string;
}

function readObject(value: unknown, path: string): JsonObject {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error(`${path} is a JSON object`);
  }
  return { fields: value as Record<string, unknown>, path };
}

function readObjectField(parent: JsonObject, key: string): JsonObject {
  return readObject(parent.fields[key], `${parent.path}.${key}`);
}

function readOptionalObject(parent: JsonObject, key: string): JsonObject | undefined {
  return parent.fields[key] === undefined ? undefined : readObjectField(parent, key);
}

function readText(object: JsonObject, key: string): string {
  const value = object.fields[key];
  if (typeof value !== 'string') {
    throw new Error(`${object.path}.${key} is text`);
  }
  return value;
}

function readBoolean(object: JsonObject, key: string): boolean {
  const value = object.fields[key];
  if (typeof value !== 'boolean') {
    throw new Error(`${object.path}.${key} is a boolean`);
  }
  return value;
}

function readOptionalText(object: JsonObject, key: string): string | undefined {
  return object.fields[key] === undefined ? undefined : readText(object, key);
}

function readInteger(object: JsonObject, key: string): number {
  const value = object.fields[key];
  if (typeof value !== 'number' || !Number.isSafeInteger(value)) {
    throw new Error(`${object.path}.${key} is a whole number this reader can represent`);
  }
  return value;
}

function readArray(parent: JsonObject, key: string): JsonObject[] {
  const value = parent.fields[key];
  if (!Array.isArray(value)) {
    throw new Error(`${parent.path}.${key} is an array`);
  }
  return value.map((item, index) => readObject(item, `${parent.path}.${key}[${index}]`));
}

function readHexBytes(object: JsonObject, key: string): Uint8Array {
  const hex = readText(object, key);
  return fromHex(hex, hex.length);
}

function readMasterKey(object: JsonObject, key: string): MasterKey {
  return MasterKey.fromBytes(fromHex(readText(object, key), MASTER_KEY_LENGTH * 2));
}

function readOneOf<T extends string>(value: string, allowed: readonly T[], key: string): T {
  const found = allowed.find((candidate) => candidate === value);
  if (found === undefined) {
    throw new Error(`${key} is one of ${allowed.join(', ')}, found ${JSON.stringify(value)}`);
  }
  return found;
}

/**
 * Narrows a 64-bit manifest value to the JSON number the schema spells it as.
 *
 * The fixtures stay well inside this range on purpose: a manifest is JSON, and
 * a value JSON cannot carry exactly is a value the exchange cannot state.
 */
function safeInteger(value: bigint, what: string): number {
  if (value > BigInt(Number.MAX_SAFE_INTEGER) || value < BigInt(Number.MIN_SAFE_INTEGER)) {
    throw new Error(`${what} is beyond what a manifest can state: ${value}`);
  }
  return Number(value);
}
