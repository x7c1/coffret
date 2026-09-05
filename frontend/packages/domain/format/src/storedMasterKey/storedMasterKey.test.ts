import { describe, expect, it } from 'vitest';

import { asyncErrorCode, errorCode } from '../errors.testing.js';
import { seal } from '../internal/aead.js';
import { MAX_FORMAT_INTEGER, asciiBytes, concatBytes, readU32BE } from '../internal/bytes.js';
import { MasterKey } from '../model/masterKey.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import {
  INITIAL_ARGON2_PARAMS,
  deriveProtectionKey,
  requireArgon2Params,
  type Argon2Params,
} from './argon2Params.js';
import {
  STORED_MASTER_KEY_SALT_LENGTH,
  StoredMasterKey,
  type StoredMasterKeyCreateRequest,
} from './storedMasterKey.js';

/**
 * A cost cheap enough to run in a test, and unlike the initial values.
 *
 * The stored form records the cost it was written at, so a test may write at a
 * cost no device would choose without saying anything about what a device
 * chooses.
 */
const CHEAP: Argon2Params = { memoryKib: 8, iterations: 1, parallelism: 1 };

const PASSPHRASE = asciiBytes('correct horse battery staple');
const MASTER_KEY = MasterKey.fromBytes(new Uint8Array(32).fill(0x3d));
const EPOCH = MasterKeyEpoch.of(4n);

function create(overrides: Partial<StoredMasterKeyCreateRequest> = {}): Promise<StoredMasterKey> {
  return StoredMasterKey.create({
    passphrase: PASSPHRASE,
    masterKey: MASTER_KEY,
    epoch: EPOCH,
    params: CHEAP,
    ...overrides,
  });
}

/**
 * A form whose eight epoch bytes spell `epoch`, sealed the way a writer would.
 *
 * `StoredMasterKey.create` takes a `MasterKeyEpoch`, so a number that names no
 * epoch cannot reach the plaintext through it. Resealing under the same
 * associated data puts one there and leaves the form authentic — so what the
 * cases below observe is the epoch's own refusal rather than a tag that failed
 * to verify.
 */
async function resealedWithEpochBytes(epoch: bigint): Promise<StoredMasterKey> {
  const stored = (await create()).bytes();
  const saltEnd = 20 + STORED_MASTER_KEY_SALT_LENGTH;
  const nonceEnd = saltEnd + 24;
  const associatedData = stored.subarray(0, nonceEnd);
  const protectionKey = await deriveProtectionKey(CHEAP, PASSPHRASE, stored.subarray(20, saltEnd));

  // The Master Key, then the eight bytes where the epoch belongs.
  const plaintext = new Uint8Array(MASTER_KEY.bytes().length + 8);
  plaintext.set(MASTER_KEY.bytes(), 0);
  new DataView(plaintext.buffer).setBigUint64(MASTER_KEY.bytes().length, epoch, false);

  return StoredMasterKey.fromBytes(
    concatBytes(
      associatedData,
      seal(protectionKey, stored.subarray(saltEnd, nonceEnd), associatedData, plaintext),
    ),
  );
}

/** The stored bytes with one of them flipped. */
function tamper(stored: StoredMasterKey, index: number, mask = 0x01): StoredMasterKey {
  const bytes = stored.bytes();
  bytes[index] ^= mask;
  return StoredMasterKey.fromBytes(bytes);
}

describe('the stored Master Key', () => {
  // KD-6: the initial values come from the OWASP-recommended band current at
  // release. Pinning them here makes a change to them a deliberate edit.
  it('ships the recommended initial Argon2id cost', () => {
    expect(INITIAL_ARGON2_PARAMS).toEqual({ memoryKib: 19_456, iterations: 2, parallelism: 1 });
  });

  // KD-9: the form is magic "CFMK1", version 0x01, a reserved byte, the salt
  // length, the three Argon2id parameters, the salt, the nonce, and the
  // ciphertext with its tag, at those exact offsets, integers big-endian.
  it('lays the form out as the field table says', async () => {
    const bytes = (await create()).bytes();
    expect(Array.from(bytes.subarray(0, 5))).toEqual(Array.from(asciiBytes('CFMK1')));
    expect(bytes[5]).toBe(0x01);
    expect(bytes[6]).toBe(0x00);
    expect(bytes[7]).toBe(STORED_MASTER_KEY_SALT_LENGTH);
    expect(readU32BE(bytes, 8)).toBe(CHEAP.memoryKib);
    expect(readU32BE(bytes, 12)).toBe(CHEAP.iterations);
    expect(readU32BE(bytes, 16)).toBe(CHEAP.parallelism);
    // 20 + S salt, 24 nonce, 40 ciphertext, 16 tag.
    expect(bytes.length).toBe(20 + STORED_MASTER_KEY_SALT_LENGTH + 24 + 40 + 16);
  });

  // KD-5, KD-7: the form round-trips under the Passphrase that protects it,
  // carrying the Master Key and the epoch it belongs to.
  it('round-trips under the correct Passphrase', async () => {
    const stored = await create();
    const unlocked = await stored.unlock(PASSPHRASE);
    expect(Array.from(unlocked.masterKey.bytes())).toEqual(Array.from(MASTER_KEY.bytes()));
    expect(unlocked.epoch.equals(EPOCH)).toBe(true);
  });

  // KD-7, FM-13: the sealed plaintext is the Master Key and the epoch it belongs
  // to, and epochs are numbered from 1 upward. A form carrying epoch 0 is
  // authentic — resealing under the same associated data leaves the tag valid —
  // so the epoch is refused on its own terms rather than by authentication.
  it('rejects a stored epoch below one', async () => {
    const forged = await resealedWithEpochBytes(0n);
    expect(await asyncErrorCode(() => forged.unlock(PASSPHRASE))).toBe('epoch_out_of_range');
  });

  // KD-7, FM-19: the eight epoch bytes spell any 64-bit number, and the ones
  // that number an epoch stop at the largest integer the format admits — so a
  // form carrying a larger one names no epoch either, the same refusal epoch 0
  // gets and for the same reason.
  it('rejects a stored epoch past the integer range the format admits', async () => {
    const forged = await resealedWithEpochBytes(MAX_FORMAT_INTEGER + 1n);
    expect(await asyncErrorCode(() => forged.unlock(PASSPHRASE))).toBe('epoch_out_of_range');

    // The bound itself is an epoch a Library can reach, so it still unlocks.
    const atTheBound = await resealedWithEpochBytes(MAX_FORMAT_INTEGER);
    expect((await atTheBound.unlock(PASSPHRASE)).epoch.value).toBe(MAX_FORMAT_INTEGER);
  });

  // KD-5, KD-6: a form written without a stated cost takes this build's initial
  // values, and unlocks at them.
  it('protects at the initial cost by default', async () => {
    const stored = await StoredMasterKey.create({
      passphrase: PASSPHRASE,
      masterKey: MASTER_KEY,
      epoch: EPOCH,
    });
    expect(stored.params).toEqual(INITIAL_ARGON2_PARAMS);
    const unlocked = await stored.unlock(PASSPHRASE);
    expect(Array.from(unlocked.masterKey.bytes())).toEqual(Array.from(MASTER_KEY.bytes()));
  });

  // KD-5: the salt is per device, so two forms of the same key under the same
  // Passphrase are different bytes.
  it('draws a fresh salt for every form', async () => {
    const first = (await create()).bytes();
    const second = (await create()).bytes();
    expect(Array.from(first)).not.toEqual(Array.from(second));
  });

  // KD-7: unlocking with the wrong Passphrase fails authentication and releases
  // nothing.
  it('fails under a wrong Passphrase', async () => {
    const stored = await create();
    expect(await asyncErrorCode(() => stored.unlock(asciiBytes('wrong passphrase')))).toBe(
      'authentication_failed',
    );
  });

  // KD-7: everything before the ciphertext is the associated data, so editing
  // the recorded parameters, the salt, or the nonce fails — a parameter
  // downgrade is detected rather than merely useless.
  it('fails when the recorded parameters, salt, or nonce are edited', async () => {
    // A cost with room above it in every parameter, so each edit below lands on
    // parameters Argon2id still accepts — an edit is caught because the bytes
    // are authenticated, not because the cost became unusable.
    const stored = await create({ params: { memoryKib: 64, iterations: 1, parallelism: 1 } });
    const edits: [string, number, number][] = [
      ['memory cost', 11, 0x01],
      ['iterations', 15, 0x02],
      ['parallelism', 19, 0x02],
      ['salt', 20, 0x01],
      ['nonce', 20 + STORED_MASTER_KEY_SALT_LENGTH, 0x01],
      ['ciphertext', 20 + STORED_MASTER_KEY_SALT_LENGTH + 24, 0x01],
      ['tag', stored.bytes().length - 1, 0x01],
    ];
    for (const [field, index, mask] of edits) {
      expect(
        await asyncErrorCode(() => tamper(stored, index, mask).unlock(PASSPHRASE)),
        field,
      ).toBe('authentication_failed');
    }
  });

  // KD-7: an edit that lands on parameters Argon2id will not run is refused as
  // well — no unlock path treats an unusable cost as an absent one.
  it('fails when an edited cost is one Argon2id refuses', async () => {
    const stored = await create();
    expect(await asyncErrorCode(() => tamper(stored, 19).unlock(PASSPHRASE))).toBe(
      'invalid_argon2_params',
    );
  });

  // KD-6, KD-7: the recorded parameters drive the derivation, so a form written
  // at another cost still unlocks — raising the cost rewrites only this form.
  it('unlocks a form written at another cost', async () => {
    const stronger: Argon2Params = { memoryKib: 32, iterations: 2, parallelism: 1 };
    const stored = await create({ params: stronger });
    expect(stored.params).toEqual(stronger);
    const unlocked = await stored.unlock(PASSPHRASE);
    expect(Array.from(unlocked.masterKey.bytes())).toEqual(Array.from(MASTER_KEY.bytes()));
  });

  // KD-9: a reader rejects an unknown magic or version, a non-zero reserved
  // byte, or a total length that disagrees with the recorded salt length.
  it('rejects bytes that are not this form', async () => {
    const stored = (await create()).bytes();

    const wrongMagic = Uint8Array.from(stored);
    wrongMagic[0] = 0x00;
    expect(errorCode(() => StoredMasterKey.fromBytes(wrongMagic))).toBe(
      'unknown_stored_master_key_magic',
    );

    const wrongVersion = Uint8Array.from(stored);
    wrongVersion[5] = 0x02;
    expect(errorCode(() => StoredMasterKey.fromBytes(wrongVersion))).toBe(
      'unsupported_stored_master_key_version',
    );

    const reserved = Uint8Array.from(stored);
    reserved[6] = 0x01;
    expect(errorCode(() => StoredMasterKey.fromBytes(reserved))).toBe('reserved_not_zero');

    const wrongSaltLength = Uint8Array.from(stored);
    wrongSaltLength[7] = STORED_MASTER_KEY_SALT_LENGTH + 1;
    expect(errorCode(() => StoredMasterKey.fromBytes(wrongSaltLength))).toBe(
      'stored_master_key_length_mismatch',
    );

    expect(errorCode(() => StoredMasterKey.fromBytes(stored.subarray(0, stored.length - 1)))).toBe(
      'stored_master_key_length_mismatch',
    );

    const extended = new Uint8Array(stored.length + 1);
    extended.set(stored, 0);
    expect(errorCode(() => StoredMasterKey.fromBytes(extended))).toBe(
      'stored_master_key_length_mismatch',
    );

    expect(errorCode(() => StoredMasterKey.fromBytes(stored.subarray(0, 8)))).toBe(
      'stored_master_key_length_mismatch',
    );
  });

  // KD-9: a reader follows the recorded salt length rather than its own build's
  // policy.
  it('follows the recorded salt length', async () => {
    const stored = await create({ salt: new Uint8Array(24).fill(0x11) });
    expect(stored.bytes()[7]).toBe(24);
    expect(stored.bytes().length).toBe(20 + 24 + 24 + 40 + 16);
    const unlocked = await stored.unlock(PASSPHRASE);
    expect(unlocked.epoch.equals(EPOCH)).toBe(true);
  });

  // KD-5: parameters Argon2id will not accept are reported as such rather than
  // producing a key.
  it('rejects parameters Argon2id refuses', () => {
    expect(errorCode(() => requireArgon2Params({ ...CHEAP, memoryKib: 0 }))).toBe(
      'invalid_argon2_params',
    );
    expect(errorCode(() => requireArgon2Params({ ...CHEAP, iterations: 0 }))).toBe(
      'invalid_argon2_params',
    );
    expect(errorCode(() => requireArgon2Params({ memoryKib: 8, iterations: 1, parallelism: 2 }))).toBe(
      'invalid_argon2_params',
    );
  });
});
