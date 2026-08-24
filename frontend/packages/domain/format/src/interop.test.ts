/**
 * The TypeScript half of the cross-implementation fixture exchange.
 *
 * coffret carries two independent implementations of the storage format: the
 * Rust crates under `backend/`, and this package. Each was written from the
 * published specification rather than from the other, so agreement between them
 * is evidence about the specification and not about a shared codebase.
 *
 * This suite is the middle step of `make interop`: the Rust `coffret-interop`
 * binary writes a fixture set, this suite opens every fixture in it and checks
 * it against the manifest, and then writes a set of its own for
 * `coffret-interop verify` to open in return.
 *
 * Without fixture directories the suite is skipped, so `pnpm -r test` stays
 * self-contained: the exchange needs a Rust build, which a package's own test
 * run has no business requiring.
 */

import { beforeAll, describe, expect, it } from 'vitest';

import {
  KeyEnvelope,
  PurposeKey,
  StoredMasterKey,
  decodeContainer,
  decodeControlObject,
  decodeIndexSnapshot,
  decodeJournalRecord,
  decodeKeyring,
  encodeContainer,
  encodeControlObject,
  encodeIndexSnapshot,
  encodeJournalRecord,
  encodeKeyring,
  generateContainerId,
  generateContainerKey,
  generateMasterKey,
  keyringSetDigest,
  parseControlObjectName,
  purposeOfControlObject,
  unwrapContainerKey,
  wrapContainerKey,
  type ContainerId,
  type ContainerKey,
  type ControlPayload,
  type EntrySource,
  type MasterKey,
} from './index.js';
import {
  BLOBS_DIR,
  FixtureReader,
  FixtureWriter,
  OBJECTS_DIR,
  REQUIRED_CONTAINERS,
  REQUIRED_CONTROL_OBJECTS,
  REQUIRED_KEY_ENVELOPES,
  REQUIRED_STORED_MASTER_KEYS,
  decodeBodyFields,
  sortBodyFields,
  type ContainerFixture,
  type ControlObjectFixture,
  type Manifest,
  type StoredMasterKeyFixture,
} from './interop.testing.js';

/** The fixture set to read, written by the other implementation. */
const INPUT = process.env.COFFRET_INTEROP_IN;

/** Where to write the set the other implementation reads back. */
const OUTPUT = process.env.COFFRET_INTEROP_OUT;

/**
 * The Passphrase the stored Master Key form this side writes is protected
 * under.
 *
 * Deliberately not ASCII: a Passphrase is the bytes a user typed, and the two
 * implementations have to agree that those bytes are its UTF-8 encoding.
 */
const PASSPHRASE = 'passe-partout ☕ 合言葉';

// Half a configuration is a misconfiguration, and a silently skipped exchange
// would look exactly like a passing one.
if ((INPUT === undefined) !== (OUTPUT === undefined)) {
  throw new Error(
    'the interop exchange needs both COFFRET_INTEROP_IN and COFFRET_INTEROP_OUT, or neither',
  );
}

describe.skipIf(INPUT === undefined || OUTPUT === undefined)('format interoperability', () => {
  let reader: FixtureReader;
  let manifest: Manifest;

  // Reading happens here rather than at collection time: a skipped suite still
  // has its body walked, and there are no fixtures to read then.
  beforeAll(() => {
    reader = new FixtureReader(required('COFFRET_INTEROP_IN', INPUT));
    manifest = reader.manifest;
  });

  it('carries every fixture kind the exchange requires', () => {
    expect(manifest.containers.map(named)).toEqual(expect.arrayContaining(REQUIRED_CONTAINERS));
    expect(manifest.controlObjects.map(named)).toEqual(
      expect.arrayContaining(REQUIRED_CONTROL_OBJECTS),
    );
    expect(manifest.keyEnvelopes.map(named)).toEqual(
      expect.arrayContaining(REQUIRED_KEY_ENVELOPES),
    );
    expect(manifest.storedMasterKeys.map(named)).toEqual(
      expect.arrayContaining(REQUIRED_STORED_MASTER_KEYS),
    );
  });

  it('opens every Container to the entries the manifest states', () => {
    for (const fixture of manifest.containers) {
      const where = `${manifest.producer}/${fixture.fixture}`;
      expect(fixture.objectName, where).toBe(fixture.containerId.objectName());

      const opened = decodeContainer(reader.read(fixture.file), fixture.containerKey);
      expect(opened.containerId.toHex(), where).toBe(fixture.containerId.toHex());
      expect(opened.kind, where).toBe(fixture.kind);
      expect(opened.chunkSize, where).toBe(fixture.chunkSize);
      expect(opened.entries.length, where).toBe(fixture.entries.length);

      // The stream layout is the one expectation the manifest does not state,
      // so it is derived here from the contents the manifest does state.
      let offset = 0n;
      for (const [index, entry] of opened.entries.entries()) {
        const stated = fixture.entries[index];
        const at = `${where} entry ${index}`;
        expect(entry.metadata.path, at).toBe(stated.path);
        expect(entry.metadata.mtimeSeconds, at).toBe(stated.mtimeSeconds);
        expect(entry.content, at).toEqual(stated.content);
        expect(entry.metadata.mime, at).toBe(stated.mime);
        expect(entry.metadata.derivedFrom?.path, at).toBe(stated.derivedFrom?.path);
        expect(entry.metadata.derivedFrom?.containerId.toHex(), at).toBe(
          stated.derivedFrom?.containerId.toHex(),
        );
        expect(entry.metadata.offset, at).toBe(offset);
        expect(entry.metadata.size, at).toBe(BigInt(stated.content.length));
        offset += BigInt(stated.content.length);
      }
    }
  });

  it('opens every control object to the payload the manifest states', () => {
    for (const fixture of manifest.controlObjects) {
      const where = `${manifest.producer}/${fixture.fixture}`;
      const key = PurposeKey.derive(manifest.masterKey, purposeOfControlObject(fixture.kind));
      // Opening is where FM-11's payload padding is checked too: the plaintext
      // has to be the CBOR map carried to its Padmé bucket with zeros. A set
      // written by an implementation that skipped the padding fails here rather
      // than travelling on with the size it leaks.
      //
      // The manifest states no length for the payload, and could not usefully:
      // the two encoders order and spell map entries as they please (which is
      // why the body below is compared as fields), so the map length a writer
      // landed on is the writer's own and not something this side derives from
      // the fields the manifest states.
      const opened = decodeControlObject(reader.read(fixture.file), fixture.objectName, key);

      expect(opened.kind, where).toBe(fixture.kind);
      expect(opened.generation.value, where).toBe(fixture.generation.value);
      expect(opened.replica.index, where).toBe(fixture.replica.index);
      expect(opened.replica.count, where).toBe(fixture.replica.count);
      expect(opened.payload.masterKeyEpoch.value, where).toBe(fixture.masterKeyEpoch.value);
      // Bodies are compared as decoded CBOR fields, never as bytes: the two
      // encoders order and spell map entries as they please.
      expect(decodeBodyFields(opened.payload.body), where).toEqual(sortBodyFields(fixture.body));

      // And the body is read again through the schema its kind owns, which the
      // field-by-field check above cannot stand in for: the canonical orders,
      // the `container` indexes, and the activation fields' agreement with the
      // header are what make a map an Index Snapshot rather than a map with the
      // right field names in it (FM-15, FM-16).
      expect(() => readPayloadSchema(fixture, opened.payload), where).not.toThrow();
    }
  });

  it('unwraps every Key Envelope to the Container Key the manifest states', () => {
    const key = PurposeKey.derive(manifest.masterKey, 'container-wrap');
    for (const fixture of manifest.keyEnvelopes) {
      const envelope = KeyEnvelope.fromBytes(reader.read(fixture.file));
      const unwrapped = unwrapContainerKey(key, fixture.containerId, envelope);
      expect(unwrapped.bytes(), `${manifest.producer}/${fixture.fixture}`).toEqual(
        fixture.containerKey.bytes(),
      );
    }
  });

  it('unlocks every stored Master Key form to the key the manifest states', async () => {
    const passphrase = utf8(manifest.passphrase);
    for (const fixture of manifest.storedMasterKeys) {
      const where = `${manifest.producer}/${fixture.fixture}`;
      const stored = StoredMasterKey.fromBytes(reader.read(fixture.file));
      // A reader follows the cost the form records rather than its own policy,
      // so the recorded cost is itself an expectation the manifest states.
      expect(stored.params, where).toEqual(fixture.argon2);

      const unlocked = await stored.unlock(passphrase);
      expect(unlocked.masterKey.bytes(), where).toEqual(fixture.masterKey.bytes());
      expect(unlocked.epoch.value, where).toBe(fixture.epoch.value);
    }
  });

  it('writes the same fixtures back for the other implementation to open', async () => {
    await writeReverseSet(reader, required('COFFRET_INTEROP_OUT', OUTPUT));
  });
});

function required(name: string, value: string | undefined): string {
  if (value === undefined) {
    throw new Error(`${name} is not set`);
  }
  return value;
}

/**
 * Writes the set this side hands back, with fresh key material.
 *
 * The fixtures are the ones the incoming manifest states, re-encoded here: the
 * exchange is about the byte forms, so the same logical content travelling back
 * under keys this side drew is exactly the reverse direction of the same test.
 * Every key, identifier, and Passphrase is drawn or chosen here rather than
 * copied, so an object that opens says something about this side's encoder and
 * nothing about the other side's. What travels back unchanged is only what the
 * manifest states as content — an Entry's plaintext, a payload field — because
 * a value surviving both encoders is exactly what the exchange checks.
 */
async function writeReverseSet(reader: FixtureReader, root: string): Promise<void> {
  const source = reader.manifest;
  const writer = new FixtureWriter(root);
  const masterKey = generateMasterKey();

  // The identities are drawn for the whole set before anything is encoded: a
  // derived Entry records the Container ID of the Entry it was produced from
  // (FM-9), and a Container rewritten in isolation could only carry over the
  // identity the incoming set drew, leaving this set stating an origin none of
  // its Containers has.
  const identities = new Map(
    source.containers.map((fixture): [string, ContainerIdentity] => [
      fixture.containerId.toHex(),
      { containerId: generateContainerId(), containerKey: generateContainerKey() },
    ]),
  );
  const containers = source.containers.map((fixture) =>
    rewriteContainer(writer, fixture, identities),
  );

  const keyEnvelopes = source.keyEnvelopes.map((fixture) => {
    // An envelope belongs to a Container, so this one wraps the re-encoded key
    // of whichever Container the incoming envelope named.
    const container = identityOf(identities, fixture.containerId);
    const envelope = wrapContainerKey(
      PurposeKey.derive(masterKey, 'container-wrap'),
      container.containerId,
      container.containerKey,
    );
    const file = writer.write(BLOBS_DIR, `${fixture.fixture}.bin`, envelope.bytes());
    return {
      ...fixture,
      file,
      containerId: container.containerId,
      containerKey: container.containerKey,
    };
  });

  const controlObjects = source.controlObjects.map((fixture) => {
    const encoded = encodeControlObject({
      // The name is parsed, not rebuilt: what a name says is the object's role,
      // and the kind travels beside it because one role admits two of them
      // (FM-12).
      name: parseControlObjectName(fixture.objectName),
      kind: fixture.kind,
      key: PurposeKey.derive(masterKey, purposeOfControlObject(fixture.kind)),
      // The incoming object is opened under the key the *other* side wrote
      // it with, and re-sealed under this side's; only the content travels.
      payload: rewritePayload(fixture, reader.read(fixture.file), source.masterKey),
    });
    return {
      ...fixture,
      file: writer.write(OBJECTS_DIR, encoded.objectName, encoded.bytes),
      objectName: encoded.objectName,
    };
  });

  const storedMasterKeys: StoredMasterKeyFixture[] = [];
  for (const fixture of source.storedMasterKeys) {
    const stored = await StoredMasterKey.create({
      passphrase: utf8(PASSPHRASE),
      masterKey,
      epoch: fixture.epoch,
      params: fixture.argon2,
    });
    storedMasterKeys.push({
      ...fixture,
      file: writer.write(BLOBS_DIR, `${fixture.fixture}.bin`, stored.bytes()),
      masterKey,
    });
  }

  writer.writeManifest({
    producer: 'typescript',
    masterKey,
    passphrase: PASSPHRASE,
    containers,
    controlObjects,
    keyEnvelopes,
    storedMasterKeys,
  });
}

/**
 * Reads one payload through the schema its kind owns, raising if it is not one.
 *
 * A Keyring is read once further, because one of its values is not in its
 * payload at all: the `set_digest` its name carries is recomputed from the
 * mapping this side decoded and held against that name (FM-17, FM-12, KL-1).
 * That is the only expectation in the exchange the manifest states outside the
 * body — and it has to be, since a payload carrying its own digest would have
 * the digest cover itself.
 */
function readPayloadSchema(fixture: ControlObjectFixture, payload: ControlPayload): void {
  switch (fixture.kind) {
    case 'journal':
      decodeJournalRecord(payload, fixture.generation);
      break;
    case 'index-snapshot':
    case 'activation-snapshot':
      decodeIndexSnapshot(payload, fixture.kind);
      break;
    case 'keyring': {
      const stated = parseControlObjectName(fixture.objectName).setDigest;
      const computed = keyringSetDigest(decodeKeyring(payload));
      if (computed !== stated) {
        throw new Error(
          `set_digest: the mapping digests to ${computed}, the name states ${stated}`,
        );
      }
      break;
    }
  }
}

/**
 * The payload this side writes back for one incoming control object.
 *
 * Each payload is decoded and encoded again through its kind's own schema, so
 * the set travelling back was written by this side's FM-15, FM-16, and FM-17
 * encoders rather than assembled from the manifest's field list — which is the
 * half of the exchange the incoming direction cannot cover.
 */
function rewritePayload(
  fixture: ControlObjectFixture,
  object: Uint8Array,
  masterKey: MasterKey,
): ControlPayload {
  const opened = decodeControlObject(
    object,
    fixture.objectName,
    PurposeKey.derive(masterKey, purposeOfControlObject(fixture.kind)),
  );
  switch (fixture.kind) {
    case 'journal':
      return encodeJournalRecord(decodeJournalRecord(opened.payload, fixture.generation));
    case 'index-snapshot':
    case 'activation-snapshot':
      return encodeIndexSnapshot(decodeIndexSnapshot(opened.payload, fixture.kind));
    case 'keyring':
      // The mapping travels unchanged, so the digest does too — which is what
      // lets the name below stay the one the incoming set used (FM-17).
      return encodeKeyring(decodeKeyring(opened.payload), fixture.masterKeyEpoch);
  }
}

/** What this side draws to stand in for one Container of the incoming set. */
interface ContainerIdentity {
  containerId: ContainerId;
  containerKey: ContainerKey;
}

/** The identity this side drew for the Container an incoming fixture names. */
function identityOf(
  identities: ReadonlyMap<string, ContainerIdentity>,
  containerId: ContainerId,
): ContainerIdentity {
  const identity = identities.get(containerId.toHex());
  if (identity === undefined) {
    throw new Error(`no Container fixture in the set is ${containerId.toHex()}`);
  }
  return identity;
}

function rewriteContainer(
  writer: FixtureWriter,
  fixture: ContainerFixture,
  identities: ReadonlyMap<string, ContainerIdentity>,
): ContainerFixture {
  const { containerId, containerKey } = identityOf(identities, fixture.containerId);
  // A derived Entry's origin follows the Container it names into this set, so
  // the parent it records is the one actually holding that Entry here (FM-9).
  const entries = fixture.entries.map((entry) =>
    entry.derivedFrom === undefined
      ? entry
      : {
          ...entry,
          derivedFrom: {
            ...entry.derivedFrom,
            containerId: identityOf(identities, entry.derivedFrom.containerId).containerId,
          },
        },
  );
  const sources: EntrySource[] = entries.map((entry) => ({
    path: entry.path,
    mtimeSeconds: entry.mtimeSeconds,
    content: entry.content,
    ...(entry.derivedFrom === undefined ? {} : { derivedFrom: entry.derivedFrom }),
    ...(entry.mime === undefined ? {} : { mime: entry.mime }),
  }));

  const encoded = encodeContainer({
    containerId,
    kind: fixture.kind,
    key: containerKey,
    chunkSize: fixture.chunkSize,
    entries: sources,
  });
  return {
    ...fixture,
    file: writer.write(OBJECTS_DIR, encoded.objectName, encoded.bytes),
    objectName: encoded.objectName,
    containerId,
    containerKey,
    entries,
  };
}

/** The bytes of a Passphrase, which is text a user typed. */
function utf8(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

function named(fixture: { fixture: string }): string {
  return fixture.fixture;
}
