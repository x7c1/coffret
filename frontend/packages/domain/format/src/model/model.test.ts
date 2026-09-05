import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { MAX_FORMAT_INTEGER } from '../internal/bytes.js';
import { ContainerId, generateContainerId } from './containerId.js';
import { ContainerKey, generateContainerKey } from './containerKey.js';
import { Generation } from './generation.js';
import { KeyEnvelope } from './keyEnvelope.js';
import { MasterKey, generateMasterKey } from './masterKey.js';
import { MasterKeyEpoch } from './masterKeyEpoch.js';
import { ReplicaPosition } from './replicaPosition.js';

describe('Container IDs', () => {
  const sample = ContainerId.fromBytes(
    Uint8Array.from([
      0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
      0xff,
    ]),
  );

  // FM-3: a Container's object name is its ID as 32 lowercase hex characters
  // followed by `.cfrt`, so the name says nothing about the content.
  it('names an object as lowercase hex plus the extension', () => {
    // The sample carries every nibble value, so this one expected string pins
    // the 32-character length and the lowercase spelling of each digit.
    expect(sample.objectName()).toBe('00112233445566778899aabbccddeeff.cfrt');
  });

  it('round-trips through hex', () => {
    expect(ContainerId.fromHex(sample.toHex()).equals(sample)).toBe(true);
  });

  it('rejects a hex spelling of the wrong length', () => {
    expect(errorCode(() => ContainerId.fromHex('00112233'))).toBe('invalid_hex_length');
  });

  it('rejects an uppercase hex spelling', () => {
    expect(errorCode(() => ContainerId.fromHex('00112233445566778899AABBCCDDEEFF'))).toBe(
      'invalid_hex_digit',
    );
  });

  it('rejects the wrong number of bytes', () => {
    expect(errorCode(() => ContainerId.fromBytes(new Uint8Array(15)))).toBe('invalid_byte_length');
  });

  // FM-3, KD-1, KD-2: identifiers and keys are drawn from the platform CSPRNG,
  // at the widths the rules give them.
  it('draws distinct identifiers and keys', () => {
    const ids = new Set(Array.from({ length: 64 }, () => generateContainerId().toHex()));
    expect(ids.size).toBe(64);
    expect(generateContainerId().bytes().length).toBe(16);
    expect(generateContainerKey().bytes().length).toBe(32);
    expect(generateMasterKey().bytes().length).toBe(32);

    const keys = new Set(Array.from({ length: 64 }, () => generateContainerKey().bytes().toString()));
    expect(keys.size).toBe(64);
  });
});

describe('key material', () => {
  it('does not leak through a formatter', () => {
    const masterKey = MasterKey.fromBytes(new Uint8Array(32).fill(0xab));
    const containerKey = ContainerKey.fromBytes(new Uint8Array(32).fill(0xab));
    expect(`${masterKey}`).toBe('MasterKey(<redacted>)');
    expect(`${containerKey}`).toBe('ContainerKey(<redacted>)');
    expect(JSON.stringify({ masterKey, containerKey })).toBe(
      '{"masterKey":"MasterKey(<redacted>)","containerKey":"ContainerKey(<redacted>)"}',
    );
  });

  it('hands out its bytes as a copy, so a holder cannot edit the key', () => {
    const key = ContainerKey.fromBytes(new Uint8Array(32).fill(0x11));
    const bytes = key.bytes();
    bytes[0] = 0xff;
    expect(key.bytes()[0]).toBe(0x11);
  });
});

describe('Key Envelopes', () => {
  it('takes exactly seventy-two bytes', () => {
    expect(KeyEnvelope.fromBytes(new Uint8Array(72)).bytes().length).toBe(72);
    expect(errorCode(() => KeyEnvelope.fromBytes(new Uint8Array(71)))).toBe('invalid_byte_length');
  });
});

describe('Master Key epochs', () => {
  // FM-13: the epoch is 1 for the Library's first epoch, incremented by 1 at
  // each rotation.
  it('starts at one and increments', () => {
    expect(MasterKeyEpoch.FIRST.value).toBe(1n);
    expect(MasterKeyEpoch.FIRST.next().value).toBe(2n);
  });

  it('is not zero', () => {
    expect(errorCode(() => MasterKeyEpoch.of(0n))).toBe('epoch_out_of_range');
  });

  // FM-19: every integer the format carries is below 2^63, so the epoch at the
  // bound is the last one a Library can rotate into and the number above it
  // names no epoch at all.
  it('refuses an epoch past the integer range the format admits', () => {
    expect(MasterKeyEpoch.of(MAX_FORMAT_INTEGER).value).toBe(MAX_FORMAT_INTEGER);
    expect(errorCode(() => MasterKeyEpoch.of(MAX_FORMAT_INTEGER + 1n))).toBe('epoch_out_of_range');
    expect(errorCode(() => MasterKeyEpoch.of(MAX_FORMAT_INTEGER).next())).toBe(
      'epoch_out_of_range',
    );
  });
});

describe('generations', () => {
  // FM-13: the head chain and the Keyring each start at generation 0 and step
  // by 1, never restarting at a rotation.
  it('counts up from the first generation', () => {
    expect(Generation.FIRST.value).toBe(0n);
    expect(Generation.FIRST.next().equals(Generation.of(1n))).toBe(true);
  });

  // FM-19: the generation at the bound is the last one a head chain can reach,
  // and the number above it names no control object.
  it('refuses a generation past the integer range the format admits', () => {
    expect(Generation.of(MAX_FORMAT_INTEGER).value).toBe(MAX_FORMAT_INTEGER);
    expect(errorCode(() => Generation.of(MAX_FORMAT_INTEGER + 1n))).toBe(
      'generation_out_of_range',
    );
    expect(errorCode(() => Generation.of(MAX_FORMAT_INTEGER).next())).toBe(
      'generation_out_of_range',
    );
  });

  it('is an unsigned number', () => {
    expect(errorCode(() => Generation.of(-1n))).toBe('generation_out_of_range');
  });
});

describe('replica positions', () => {
  // FM-12: Journal records and Index Snapshots use replica index 0, count 1.
  it('is replica zero of one when written once', () => {
    expect(ReplicaPosition.SINGLE.index).toBe(0);
    expect(ReplicaPosition.SINGLE.count).toBe(1);
    expect(ReplicaPosition.of(0, 1).equals(ReplicaPosition.SINGLE)).toBe(true);
  });

  it('rejects an index outside the count', () => {
    expect(errorCode(() => ReplicaPosition.of(3, 3))).toBe('invalid_replica_position');
    expect(errorCode(() => ReplicaPosition.of(0, 0))).toBe('invalid_replica_position');
  });
});
