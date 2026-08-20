import { describe, expect, it } from 'vitest';

import { errorCode } from './errors.testing.js';
import { open, seal } from './internal/aead.js';
import { metaNonce } from './internal/nonce.js';
import { MasterKey } from './model/masterKey.js';
import { ContainerKey } from './model/containerKey.js';
import {
  PURPOSES,
  PURPOSE_INFO,
  PurposeKey,
  purposeKeyBytes,
  purposeOfControlObject,
  type Purpose,
} from './purposeKey.js';

/**
 * A Master Key whose every byte differs, so a derivation that dropped or
 * reordered input bytes would not land on the same output.
 */
function masterKey(): MasterKey {
  return MasterKey.fromBytes(Uint8Array.from({ length: 32 }, (_, index) => index));
}

function derived(purpose: Purpose): Uint8Array {
  return purposeKeyBytes(PurposeKey.derive(masterKey(), purpose), purpose);
}

describe('purpose keys', () => {
  // KD-4: the v1 purpose registry and the info string each purpose derives
  // under.
  it('spells the info strings the registry lists', () => {
    expect(PURPOSE_INFO['container-wrap']).toBe('coffret/v1/container-wrap');
    expect(PURPOSE_INFO['control/journal']).toBe('coffret/v1/control/journal');
    expect(PURPOSE_INFO['control/keyring']).toBe('coffret/v1/control/keyring');
    expect(PURPOSE_INFO['control/index-snapshot']).toBe('coffret/v1/control/index-snapshot');
    expect(PURPOSE_INFO['token-cache']).toBe('coffret/v1/token-cache');
  });

  // KD-4: a key derived for one purpose is used for no other, so no two
  // purposes may share an info string.
  it('gives every purpose its own info string', () => {
    const infos = PURPOSES.map((purpose) => PURPOSE_INFO[purpose]);
    expect(new Set(infos).size).toBe(PURPOSES.length);
  });

  // KD-4, FM-11: each control-object kind is encrypted under its own purpose.
  it('maps each control-object kind to its own purpose', () => {
    expect(purposeOfControlObject('journal')).toBe('control/journal');
    expect(purposeOfControlObject('keyring')).toBe('control/keyring');
    expect(purposeOfControlObject('index-snapshot')).toBe('control/index-snapshot');
  });

  // KD-3, KD-4: purpose keys are HKDF-SHA-256 over the Master Key with a
  // zero-length salt, the purpose's info string, and a 32-byte output. These are
  // the vectors the Rust implementation pins for the Master Key above; the two
  // implementations landing on the same bytes is what makes the derivation a
  // property of the specification rather than of either build.
  it('derives the pinned vectors', () => {
    expect(Array.from(derived('container-wrap'))).toEqual([
      0xef, 0x89, 0x47, 0xe4, 0xd7, 0x83, 0x1b, 0xe5, 0xc1, 0x89, 0x44, 0x89, 0xe2, 0xfa, 0x1e,
      0x6a, 0xd0, 0xf3, 0x5e, 0x84, 0xbe, 0x80, 0x55, 0x2c, 0x81, 0x0b, 0x44, 0xe4, 0x05, 0x8b,
      0xe5, 0x1b,
    ]);
    expect(Array.from(derived('control/journal'))).toEqual([
      0xb3, 0xef, 0x1d, 0x17, 0x4a, 0x07, 0xe6, 0xeb, 0xc7, 0x30, 0x90, 0xad, 0x90, 0x8a, 0x36,
      0x18, 0xbe, 0x34, 0x84, 0x0c, 0x45, 0xf8, 0x85, 0x28, 0x31, 0x58, 0x69, 0x4a, 0x95, 0x49,
      0x60, 0x40,
    ]);
    expect(Array.from(derived('control/keyring'))).toEqual([
      0x92, 0x16, 0x29, 0xb1, 0x9a, 0x4d, 0xfc, 0xa1, 0x69, 0x32, 0x01, 0xfe, 0x25, 0xc6, 0xd5,
      0xaa, 0x90, 0x15, 0x0f, 0xae, 0x50, 0x35, 0x92, 0xae, 0xe0, 0x8f, 0x4d, 0x1d, 0x70, 0xdc,
      0x6f, 0x1d,
    ]);
    expect(Array.from(derived('control/index-snapshot'))).toEqual([
      0x10, 0xd7, 0x0a, 0xdb, 0xee, 0x11, 0xad, 0x0f, 0xb7, 0x19, 0x09, 0x42, 0xc7, 0x92, 0x3b,
      0xe2, 0xaa, 0xe9, 0xf4, 0xf5, 0x0d, 0xfd, 0x29, 0xee, 0xf5, 0x69, 0xdb, 0xe4, 0x8b, 0xd8,
      0xe2, 0x5c,
    ]);
    expect(Array.from(derived('token-cache'))).toEqual([
      0xde, 0x5b, 0x77, 0xda, 0x95, 0x08, 0x82, 0x1a, 0x4f, 0x96, 0x51, 0xad, 0xe2, 0x24, 0x93,
      0xc3, 0x99, 0xb4, 0xc0, 0xa6, 0x87, 0xff, 0x27, 0x54, 0x25, 0xd2, 0x28, 0xd8, 0x1c, 0x39,
      0x1f, 0x75,
    ]);
  });

  // KD-3: every purpose key is 32 bytes, the output length the rule states.
  it('derives 32-byte keys', () => {
    for (const purpose of PURPOSES) {
      expect(derived(purpose).length).toBe(32);
    }
  });

  // KD-3: the Master Key is never used directly as an AEAD key — every purpose
  // key differs from it and from every other purpose key.
  it('repeats neither another purpose key nor the Master Key', () => {
    const keys = PURPOSES.map((purpose) => derived(purpose).toString());
    expect(new Set(keys).size).toBe(PURPOSES.length);
    expect(keys).not.toContain(masterKey().bytes().toString());
  });

  // KD-3: derivation is deterministic — the same Master Key and purpose always
  // yield the same key, which is what lets any device open what another wrote.
  it('derives deterministically', () => {
    for (const purpose of PURPOSES) {
      expect(derived(purpose)).toEqual(derived(purpose));
    }
  });

  // KD-4: a payload sealed under one purpose key opens under no other purpose
  // key, and not under a Container Key either — the separation is
  // cryptographic, not just a label this package checks.
  it('seals under one purpose key and opens under no other', () => {
    const sealedUnder: Purpose = 'control/journal';
    const nonce = metaNonce();
    const associatedData = Uint8Array.from([0xad]);
    const message = seal(derived(sealedUnder), nonce, associatedData, Uint8Array.from([1, 2, 3]));

    const wrongKeys = PURPOSES.filter((purpose) => purpose !== sealedUnder).map(derived);
    // A Container Key is drawn independently of the Master Key (KD-2), so it is
    // no more able to open this than a wrong purpose key is.
    wrongKeys.push(ContainerKey.fromBytes(new Uint8Array(32).fill(0x11)).bytes());
    for (const key of wrongKeys) {
      expect(errorCode(() => open(key, nonce, associatedData, message))).toBe(
        'authentication_failed',
      );
    }
    expect(Array.from(open(derived(sealedUnder), nonce, associatedData, message))).toEqual([
      1, 2, 3,
    ]);
  });

  // KD-4: a key derived for one purpose is not accepted for another.
  it('refuses a purpose it was not derived for', () => {
    const key = PurposeKey.derive(masterKey(), 'control/journal');
    expect(errorCode(() => purposeKeyBytes(key, 'control/keyring'))).toBe('wrong_purpose_key');
    expect(purposeKeyBytes(key, 'control/journal').length).toBe(32);
  });

  it('does not leak key material through a formatter', () => {
    const key = PurposeKey.derive(masterKey(), 'container-wrap');
    expect(`${key}`).toBe('PurposeKey(coffret/v1/container-wrap, <redacted>)');
    expect(JSON.stringify({ key })).toBe(
      '{"key":"PurposeKey(coffret/v1/container-wrap, <redacted>)"}',
    );
  });
});
