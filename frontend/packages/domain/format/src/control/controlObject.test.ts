import { encode as encodeCborValue } from 'cborg';
import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { seal } from '../internal/aead.js';
import { asciiBytes, concatBytes } from '../internal/bytes.js';
import { randomNonce } from '../internal/nonce.js';
import { Generation } from '../model/generation.js';
import { MasterKey } from '../model/masterKey.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import { ReplicaPosition } from '../model/replicaPosition.js';
import { CONTROL_OBJECT_KINDS, type ControlObjectKind } from '../model/kinds.js';
import { PurposeKey, purposeKeyBytes, purposeOfControlObject } from '../purposeKey.js';
import { decodeControlObject } from './decode.js';
import { encodeControlObject } from './encode.js';
import { CONTROL_HEADER_LENGTH, encodeControlHeader } from './header.js';
import {
  formatControlObjectName,
  headName,
  indexSnapshotName,
  keyringReplicaName,
  nameAdmitsKind,
  type ControlObjectName,
} from './objectName.js';
import { emptyPayloadBody, type ControlPayload } from './payload.js';

const MASTER_KEY = MasterKey.fromBytes(new Uint8Array(32).fill(0x7c));
const SET_DIGEST = '9f0c';
const GENERATION = Generation.of(6n);

function key(kind: ControlObjectKind): PurposeKey {
  return PurposeKey.derive(MASTER_KEY, purposeOfControlObject(kind));
}

/**
 * The name a control object of `kind` is stored under in these tests.
 *
 * One generation throughout, so a test that swaps two names is swapping the
 * name form and nothing else. Both head-chain kinds land on the same name,
 * which is the point of FM-12's admission table.
 */
function name(kind: ControlObjectKind): ControlObjectName {
  switch (kind) {
    case 'journal':
    case 'activation-snapshot':
      return headName(GENERATION);
    case 'index-snapshot':
      return indexSnapshotName(GENERATION);
    case 'keyring':
      return keyringReplicaName(GENERATION, SET_DIGEST, ReplicaPosition.of(1, 3));
  }
}

/** Every name form FM-12 defines, all at the generation above. */
function allNameForms(): ControlObjectName[] {
  return [name('journal'), name('index-snapshot'), name('keyring')];
}

/** A payload body standing in for the fields a kind will carry later. */
function body(): Uint8Array {
  return encodeCborValue(new Map<string, unknown>([['placeholder', 1]]));
}

function payload(): ControlPayload {
  return { masterKeyEpoch: MasterKeyEpoch.of(2n), body: body() };
}

function encoded(kind: ControlObjectKind) {
  return encodeControlObject({ name: name(kind), kind, key: key(kind), payload: payload() });
}

/**
 * Frames `plaintext` as the payload of a `kind` object called `name`, whatever
 * it holds.
 *
 * The encoder builds its payload map itself, so a test that needs a payload the
 * encoder would not write — one missing a field it always adds, say — has to
 * seal the bytes here instead.
 */
function sealPayload(
  objectName: ControlObjectName,
  kind: ControlObjectKind,
  plaintext: Uint8Array,
): Uint8Array {
  const nonce = randomNonce();
  const header = encodeControlHeader({
    kind,
    generation: objectName.generation,
    replica: objectName.replica,
    nonce,
  });
  const keyBytes = purposeKeyBytes(key(kind), purposeOfControlObject(kind));
  return concatBytes(header, seal(keyBytes, nonce, header, plaintext));
}

describe('control objects', () => {
  // FM-11: the header is magic "CFCTL", format version 0x01, the kind byte, a
  // reserved byte, the generation, the replica index and count, and the nonce,
  // at those exact offsets, with multi-byte integers big-endian.
  it('lays the header out as the field table says', () => {
    const object = encoded('keyring').bytes;
    expect(Array.from(object.subarray(0, 5))).toEqual(Array.from(asciiBytes('CFCTL')));
    expect(object[5]).toBe(0x01);
    expect(object[6]).toBe(0x02);
    expect(object[7]).toBe(0x00);
    expect(Array.from(object.subarray(8, 16))).toEqual([0, 0, 0, 0, 0, 0, 0, 6]);
    expect(Array.from(object.subarray(16, 18))).toEqual([0, 1]);
    expect(Array.from(object.subarray(18, 20))).toEqual([0, 3]);
    expect(object.length).toBeGreaterThan(CONTROL_HEADER_LENGTH);
  });

  // FM-11: the kind byte is 0x01 for a Journal record, 0x02 for a Keyring
  // replica, 0x03 for an ordinary Index Snapshot, and 0x04 for the Index
  // Snapshot that activates an epoch.
  it('writes the kind byte the rule assigns', () => {
    expect(encoded('journal').bytes[6]).toBe(0x01);
    expect(encoded('keyring').bytes[6]).toBe(0x02);
    expect(encoded('index-snapshot').bytes[6]).toBe(0x03);
    expect(encoded('activation-snapshot').bytes[6]).toBe(0x04);
  });

  // FM-12: every row of the admission table round-trips under the name form it
  // lists — including both head-chain kinds, which share one name.
  it('stores each kind under the name form its role takes', () => {
    expect(encoded('journal').objectName).toBe('head-6.cfrt');
    expect(encoded('activation-snapshot').objectName).toBe('head-6.cfrt');
    expect(encoded('index-snapshot').objectName).toBe('idx-6.cfrt');
    expect(encoded('keyring').objectName).toBe(`key-6-${SET_DIGEST}-r1-of-3.cfrt`);
  });

  // FM-12: a `head-` name says which position in the chain an object occupies
  // and nothing about which kind fills it, so the kind an object opens as is the
  // one its header carries.
  it('opens one head name as either chain kind', () => {
    for (const kind of ['journal', 'activation-snapshot'] as const) {
      const object = encoded(kind);
      expect(object.objectName).toBe('head-6.cfrt');
      expect(decodeControlObject(object.bytes, 'head-6.cfrt', key(kind)).kind).toBe(kind);
    }
  });

  // FM-11, FM-13: every kind round-trips through the framing, payload and epoch
  // intact.
  it('round-trips every kind', () => {
    for (const kind of CONTROL_OBJECT_KINDS) {
      const object = encoded(kind);
      const decoded = decodeControlObject(object.bytes, object.objectName, key(kind));
      expect(decoded.kind).toBe(kind);
      expect(decoded.generation.equals(GENERATION)).toBe(true);
      expect(decoded.payload.masterKeyEpoch.equals(MasterKeyEpoch.of(2n))).toBe(true);
      expect(Array.from(decoded.payload.body)).toEqual(Array.from(body()));
    }
  });

  // FM-12: Journal records and Index Snapshots use replica index 0, count 1, and
  // a Keyring replica carries the position its name spells.
  it('carries the replica position of its kind', () => {
    for (const kind of ['journal', 'activation-snapshot', 'index-snapshot'] as const) {
      const object = encoded(kind);
      const decoded = decodeControlObject(object.bytes, object.objectName, key(kind));
      expect(decoded.replica.index).toBe(0);
      expect(decoded.replica.count).toBe(1);
    }
    const keyring = encoded('keyring');
    const decoded = decodeControlObject(keyring.bytes, keyring.objectName, key('keyring'));
    expect(decoded.replica.index).toBe(1);
    expect(decoded.replica.count).toBe(3);
  });

  // FM-11: the associated data is the full 44-byte header, so editing any field
  // of it fails — by authentication where the field is not read back, and by the
  // name check where it is.
  it('refuses an object whose header was edited', () => {
    const object = encoded('keyring');
    const refusals = [
      'authentication_failed',
      'object_name_mismatch',
      'control_object_kind_not_admitted',
      'unknown_control_magic',
      'unsupported_control_version',
      'unknown_control_object_kind',
      'reserved_not_zero',
      'invalid_replica_position',
      'wrong_purpose_key',
    ];
    for (let index = 0; index < CONTROL_HEADER_LENGTH; index++) {
      const tampered = Uint8Array.from(object.bytes);
      tampered[index] ^= 0x01;
      expect(refusals, `byte ${index} of the header was not covered`).toContain(
        errorCode(() => decodeControlObject(tampered, object.objectName, key('keyring'))),
      );
    }
    // The nonce is not read back anywhere, so it is authentication alone that
    // catches an edit to it.
    for (let index = 20; index < CONTROL_HEADER_LENGTH; index++) {
      const tampered = Uint8Array.from(object.bytes);
      tampered[index] ^= 0x01;
      expect(
        errorCode(() => decodeControlObject(tampered, object.objectName, key('keyring'))),
        `nonce byte ${index}`,
      ).toBe('authentication_failed');
    }
  });

  // FM-1: a payload that fails authentication is rejected whole.
  it('refuses an object whose payload was edited', () => {
    const object = encoded('journal');
    for (let index = CONTROL_HEADER_LENGTH; index < object.bytes.length; index++) {
      const tampered = Uint8Array.from(object.bytes);
      tampered[index] ^= 0x01;
      expect(
        errorCode(() => decodeControlObject(tampered, object.objectName, key('journal'))),
        `byte ${index}`,
      ).toBe('authentication_failed');
    }
  });

  // FM-12: an object whose name-encoded generation or replica position
  // disagrees with its header is rejected.
  it('refuses a name that disagrees with the header', () => {
    const object = encoded('keyring');
    const disagreements = [
      formatControlObjectName(keyringReplicaName(Generation.of(7n), SET_DIGEST, ReplicaPosition.of(1, 3))),
      formatControlObjectName(keyringReplicaName(GENERATION, SET_DIGEST, ReplicaPosition.of(2, 3))),
    ];
    for (const objectName of disagreements) {
      expect(
        errorCode(() => decodeControlObject(object.bytes, objectName, key('keyring'))),
        objectName,
      ).toBe('object_name_mismatch');
    }
  });

  // FM-12: the admission table decides which kind each name form may carry, and
  // every pairing outside it is refused. Each of the twelve pairings of a name
  // form with a kind is visited: the four the table lists open, the other eight
  // are refused before the payload is touched.
  it('refuses every pairing outside the admission table', () => {
    for (const kind of CONTROL_OBJECT_KINDS) {
      const object = encoded(kind);
      for (const objectName of allNameForms()) {
        const presented = formatControlObjectName(objectName);
        const where = `${presented} and ${kind}`;
        if (nameAdmitsKind(objectName, kind)) {
          expect(decodeControlObject(object.bytes, presented, key(kind)).kind, where).toBe(kind);
          continue;
        }
        expect(
          errorCode(() => decodeControlObject(object.bytes, presented, key(kind))),
          where,
        ).toBe('control_object_kind_not_admitted');
      }
    }
  });

  // FM-12: the encoder is held to the same table as the decoder, so nothing is
  // ever written under a name that would refuse it on the way back in.
  it('refuses to encode a kind under a name that does not admit it', () => {
    expect(
      errorCode(() =>
        encodeControlObject({
          name: indexSnapshotName(GENERATION),
          kind: 'journal',
          key: key('journal'),
          payload: payload(),
        }),
      ),
    ).toBe('control_object_kind_not_admitted');
  });

  // FM-11, FM-12: the two head-chain kinds share a name, so nothing about the
  // name separates them — the kind byte is authenticated and each kind has its
  // own purpose key (KD-4), and a Journal record passed off as an epoch
  // activation fails on both counts without its name changing at all.
  it('refuses a Journal record refiled as an activation', () => {
    const object = encoded('journal');
    const refiled = Uint8Array.from(object.bytes);
    refiled[6] = 0x04;
    expect(
      errorCode(() => decodeControlObject(refiled, 'head-6.cfrt', key('activation-snapshot'))),
    ).toBe('authentication_failed');
  });

  // FM-11: the payload is encrypted with the purpose key of the header's kind,
  // so another kind's key opens nothing.
  it('opens only under the purpose key of its kind', () => {
    const object = encoded('journal');
    expect(
      errorCode(() => decodeControlObject(object.bytes, object.objectName, key('keyring'))),
    ).toBe('wrong_purpose_key');
    expect(
      errorCode(() =>
        encodeControlObject({
          name: name('journal'),
          kind: 'journal',
          key: key('keyring'),
          payload: payload(),
        }),
      ),
    ).toBe('wrong_purpose_key');
  });

  // FM-13: every control-object payload carries `master_key_epoch`, and one that
  // does not is rejected.
  it('refuses a payload that does not name the Master Key epoch', () => {
    const objectName = headName(GENERATION);
    const object = sealPayload(objectName, 'journal', encodeCborValue(new Map([['records', 2]])));
    expect(
      errorCode(() =>
        decodeControlObject(object, formatControlObjectName(objectName), key('journal')),
      ),
    ).toBe('missing_master_key_epoch');
  });

  // FM-13: epoch numbering starts at 1, so a payload claiming epoch 0 names no
  // Master Key.
  it('refuses an epoch below one', () => {
    const objectName = headName(GENERATION);
    const object = sealPayload(
      objectName,
      'journal',
      encodeCborValue(new Map<string, unknown>([['master_key_epoch', 0]])),
    );
    expect(
      errorCode(() =>
        decodeControlObject(object, formatControlObjectName(objectName), key('journal')),
      ),
    ).toBe('epoch_out_of_range');
  });

  it('refuses a payload that is not a CBOR map', () => {
    const objectName = headName(GENERATION);
    const object = sealPayload(objectName, 'journal', encodeCborValue('not a map'));
    expect(
      errorCode(() =>
        decodeControlObject(object, formatControlObjectName(objectName), key('journal')),
      ),
    ).toBe('control_payload_not_a_map');
  });

  // The framing owns `master_key_epoch`; a body that also carries one would
  // leave two answers to which epoch wrote the object.
  it('refuses a body that claims the epoch field', () => {
    expect(
      errorCode(() =>
        encodeControlObject({
          name: name('journal'),
          kind: 'journal',
          key: key('journal'),
          payload: {
            masterKeyEpoch: MasterKeyEpoch.FIRST,
            body: encodeCborValue(new Map<string, unknown>([['master_key_epoch', 9]])),
          },
        }),
      ),
    ).toBe('malformed_control_payload');
  });

  it('round-trips a payload with no fields of its own', () => {
    const objectName = headName(Generation.FIRST);
    const object = encodeControlObject({
      name: objectName,
      kind: 'journal',
      key: key('journal'),
      payload: { masterKeyEpoch: MasterKeyEpoch.FIRST, body: emptyPayloadBody() },
    });
    const decoded = decodeControlObject(object.bytes, object.objectName, key('journal'));
    expect(Array.from(decoded.payload.body)).toEqual(Array.from(emptyPayloadBody()));
    expect(decoded.payload.masterKeyEpoch.equals(MasterKeyEpoch.FIRST)).toBe(true);
  });

  // FM-11: an object that is not a control object v1 is rejected on its
  // plaintext bytes, before a key is used at all.
  it('rejects an object that is not a control object v1', () => {
    const object = encoded('journal');
    const objectName = object.objectName;

    expect(
      errorCode(() =>
        decodeControlObject(object.bytes.subarray(0, 20), objectName, key('journal')),
      ),
    ).toBe('control_header_too_short');

    const wrongMagic = Uint8Array.from(object.bytes);
    wrongMagic[0] = 0x00;
    expect(errorCode(() => decodeControlObject(wrongMagic, objectName, key('journal')))).toBe(
      'unknown_control_magic',
    );

    const wrongVersion = Uint8Array.from(object.bytes);
    wrongVersion[5] = 0x02;
    expect(errorCode(() => decodeControlObject(wrongVersion, objectName, key('journal')))).toBe(
      'unsupported_control_version',
    );

    const unknownKind = Uint8Array.from(object.bytes);
    unknownKind[6] = 0x05;
    expect(errorCode(() => decodeControlObject(unknownKind, objectName, key('journal')))).toBe(
      'unknown_control_object_kind',
    );

    const reserved = Uint8Array.from(object.bytes);
    reserved[7] = 0x01;
    expect(errorCode(() => decodeControlObject(reserved, objectName, key('journal')))).toBe(
      'reserved_not_zero',
    );

    const headerOnly = object.bytes.subarray(0, CONTROL_HEADER_LENGTH);
    expect(errorCode(() => decodeControlObject(headerOnly, objectName, key('journal')))).toBe(
      'missing_control_payload',
    );
  });

  // FM-11: the nonce is drawn fresh for every object, so two writes of the same
  // payload never repeat one under the same key.
  it('gives every object its own nonce', () => {
    const first = encoded('journal').bytes.subarray(20, CONTROL_HEADER_LENGTH);
    const second = encoded('journal').bytes.subarray(20, CONTROL_HEADER_LENGTH);
    expect(Array.from(first)).not.toEqual(Array.from(second));
  });

  // The nonce may be supplied, so a fixture written by another implementation
  // can be reproduced byte for byte.
  it('encodes under a caller-supplied nonce', () => {
    const nonce = new Uint8Array(24).fill(0x5a);
    const request = {
      name: name('journal'),
      kind: 'journal' as const,
      key: key('journal'),
      payload: payload(),
      nonce,
    };
    const first = encodeControlObject(request).bytes;
    const second = encodeControlObject(request).bytes;
    expect(Array.from(first)).toEqual(Array.from(second));
    expect(Array.from(first.subarray(20, CONTROL_HEADER_LENGTH))).toEqual(Array.from(nonce));
  });
});
