import { describe, expect, it } from 'vitest';

import { errorCode } from './errors.testing.js';
import { unwrapContainerKey, wrapContainerKey } from './keyEnvelope.js';
import { ContainerId } from './model/containerId.js';
import { ContainerKey, generateContainerKey } from './model/containerKey.js';
import { KEY_ENVELOPE_LENGTH, KeyEnvelope } from './model/keyEnvelope.js';
import { MasterKey } from './model/masterKey.js';
import { PURPOSES, PurposeKey } from './purposeKey.js';

const MASTER_KEY = MasterKey.fromBytes(new Uint8Array(32).fill(0x5e));

function containerWrapKey(): PurposeKey {
  return PurposeKey.derive(MASTER_KEY, 'container-wrap');
}

function containerId(byte: number): ContainerId {
  return ContainerId.fromBytes(new Uint8Array(16).fill(byte));
}

describe('Key Envelopes', () => {
  // FM-14: a Key Envelope is nonce(24) ‖ ciphertext(32) ‖ tag(16) — 72 bytes.
  it('is seventy-two bytes', () => {
    const envelope = wrapContainerKey(
      containerWrapKey(),
      containerId(1),
      ContainerKey.fromBytes(new Uint8Array(32).fill(0x11)),
    );
    expect(envelope.bytes().length).toBe(72);
    expect(KEY_ENVELOPE_LENGTH).toBe(24 + 32 + 16);
  });

  // FM-14: the envelope carries the Container Key, and unwrapping it under the
  // same purpose key and Container ID returns exactly that key.
  it('round-trips the Container Key', () => {
    const containerKey = generateContainerKey();
    const envelope = wrapContainerKey(containerWrapKey(), containerId(2), containerKey);
    const opened = unwrapContainerKey(containerWrapKey(), containerId(2), envelope);
    expect(Array.from(opened.bytes())).toEqual(Array.from(containerKey.bytes()));
  });

  // FM-14: the nonce is fresh for every envelope, so wrapping the same key for
  // the same Container twice produces two different envelopes.
  it('gives every envelope its own nonce', () => {
    const containerKey = ContainerKey.fromBytes(new Uint8Array(32).fill(0x33));
    const first = wrapContainerKey(containerWrapKey(), containerId(3), containerKey);
    const second = wrapContainerKey(containerWrapKey(), containerId(3), containerKey);
    expect(first.equals(second)).toBe(false);
  });

  // FM-14: an envelope presented for a different Container fails to unwrap, so
  // envelopes cannot be swapped between Containers.
  it('does not open for another Container', () => {
    const envelope = wrapContainerKey(
      containerWrapKey(),
      containerId(4),
      ContainerKey.fromBytes(new Uint8Array(32).fill(0x44)),
    );
    expect(
      errorCode(() => unwrapContainerKey(containerWrapKey(), containerId(5), envelope)),
    ).toBe('authentication_failed');
  });

  // FM-1: a message that fails authentication is rejected whole — every byte of
  // an envelope is covered.
  it('rejects a tampered envelope', () => {
    const envelope = wrapContainerKey(
      containerWrapKey(),
      containerId(6),
      ContainerKey.fromBytes(new Uint8Array(32).fill(0x55)),
    );
    for (let index = 0; index < KEY_ENVELOPE_LENGTH; index++) {
      const bytes = envelope.bytes();
      bytes[index] ^= 0x01;
      expect(
        errorCode(() =>
          unwrapContainerKey(containerWrapKey(), containerId(6), KeyEnvelope.fromBytes(bytes)),
        ),
        `byte ${index} was not authenticated`,
      ).toBe('authentication_failed');
    }
  });

  // KD-4: the container-wrap key wraps Container Keys and nothing else does — a
  // key derived for another purpose is refused outright.
  it('accepts only the container-wrap purpose key', () => {
    for (const purpose of PURPOSES) {
      if (purpose === 'container-wrap') {
        continue;
      }
      const key = PurposeKey.derive(MASTER_KEY, purpose);
      expect(
        errorCode(() =>
          wrapContainerKey(key, containerId(7), ContainerKey.fromBytes(new Uint8Array(32))),
        ),
      ).toBe('wrong_purpose_key');
    }
  });

  // The nonce may be supplied, so a fixture written by another implementation
  // can be reproduced byte for byte.
  it('wraps under a caller-supplied nonce', () => {
    const nonce = new Uint8Array(24).fill(0x9a);
    const containerKey = ContainerKey.fromBytes(new Uint8Array(32).fill(0x66));
    const first = wrapContainerKey(containerWrapKey(), containerId(8), containerKey, nonce);
    const second = wrapContainerKey(containerWrapKey(), containerId(8), containerKey, nonce);
    expect(first.equals(second)).toBe(true);
    expect(Array.from(first.bytes().subarray(0, 24))).toEqual(Array.from(nonce));
  });
});
