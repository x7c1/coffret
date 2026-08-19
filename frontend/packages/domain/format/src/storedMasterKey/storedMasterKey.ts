/**
 * The form a device's Master Key takes at rest, under its Passphrase.
 *
 * The byte layout is normative in KD-9; this module implements it. Everything
 * before the ciphertext is the associated data, so the recorded Argon2id
 * parameters and the salt are authenticated: unlocking detects both tampering
 * and an attempt to talk the reader into a cheaper derivation than the writer
 * used (KD-7).
 *
 * The form is self-contained and portable — unlocking needs only these bytes and
 * the Passphrase — and it never reaches Storage: nothing Passphrase-derived does
 * (KD-8). This module deals in bytes only; where a device keeps them is a
 * question for the layer that does I/O.
 */

import { TAG_LENGTH, open, seal } from '../internal/aead.js';
import {
  asciiBytes,
  bytesEqual,
  concatBytes,
  readU32BE,
  readU64BE,
  takeExactly,
  writeU32BE,
  writeU64BE,
} from '../internal/bytes.js';
import { drawBytes } from '../internal/entropy.js';
import { NONCE_LENGTH, randomNonce } from '../internal/nonce.js';
import { fail } from '../errors.js';
import { MASTER_KEY_LENGTH, MasterKey } from '../model/masterKey.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import {
  INITIAL_ARGON2_PARAMS,
  deriveProtectionKey,
  requireArgon2Params,
  type Argon2Params,
} from './argon2Params.js';

/** The bytes a stored Master Key starts with. */
export const STORED_MASTER_KEY_MAGIC = asciiBytes('CFMK1');

/** The stored Master Key version this package writes and reads. */
export const STORED_MASTER_KEY_VERSION = 0x01;

/** Length of the salt this build draws, in bytes. */
export const STORED_MASTER_KEY_SALT_LENGTH = 16;

const VERSION_OFFSET = 5;
const RESERVED_OFFSET = 6;
const SALT_LENGTH_OFFSET = 7;
const MEMORY_KIB_OFFSET = 8;
const ITERATIONS_OFFSET = 12;
const PARALLELISM_OFFSET = 16;
/** Where the salt starts, and therefore how long the fixed part is. */
const SALT_OFFSET = 20;

/** The plaintext this form encrypts: the key, then its epoch as 8 big-endian bytes. */
const PLAINTEXT_LENGTH = MASTER_KEY_LENGTH + 8;

/** What the plaintext part of one stored form says about the rest of it. */
interface Layout {
  params: Argon2Params;
  saltEnd: number;
  nonceEnd: number;
  messageEnd: number;
}

/** What a stored Master Key yields once the Passphrase opens it. */
export interface UnlockedMasterKey {
  /** The Master Key this device holds. */
  masterKey: MasterKey;
  /** The epoch that key belongs to. */
  epoch: MasterKeyEpoch;
}

/** What creating a stored form needs. */
export interface StoredMasterKeyCreateRequest {
  /** The device Passphrase, as the bytes the user typed. */
  passphrase: Uint8Array;
  /** The Master Key to protect. */
  masterKey: MasterKey;
  /** The epoch that key belongs to. */
  epoch: MasterKeyEpoch;
  /** The Argon2id cost to write at; this build's initial values by default. */
  params?: Argon2Params;
  /** The salt to stretch under; drawn from the CSPRNG when left out. */
  salt?: Uint8Array;
  /** The nonce to seal under; drawn from the CSPRNG when left out. */
  nonce?: Uint8Array;
}

/** A Master Key protected by a Passphrase, as the bytes a device stores. */
export class StoredMasterKey {
  readonly #bytes: Uint8Array;
  readonly #layout: Layout;

  private constructor(bytes: Uint8Array, layout: Layout) {
    this.#bytes = bytes;
    this.#layout = layout;
  }

  /**
   * Protects a Master Key under a Passphrase.
   *
   * The salt is per device and per stored form; nothing outside this call needs
   * to choose it, so it is drawn here unless a caller reproducing a fixture
   * supplies one.
   */
  static async create(request: StoredMasterKeyCreateRequest): Promise<StoredMasterKey> {
    const params = requireArgon2Params(request.params ?? INITIAL_ARGON2_PARAMS);
    const salt = request.salt ?? drawBytes(STORED_MASTER_KEY_SALT_LENGTH);
    const nonce = takeExactly(request.nonce ?? randomNonce(), NONCE_LENGTH, 'a nonce');
    if (salt.length === 0 || salt.length > 0xff) {
      fail('invalid_byte_length', `a salt is 1 to 255 bytes, found ${salt.length}`);
    }

    const fixed = new Uint8Array(SALT_OFFSET);
    fixed.set(STORED_MASTER_KEY_MAGIC, 0);
    fixed[VERSION_OFFSET] = STORED_MASTER_KEY_VERSION;
    fixed[SALT_LENGTH_OFFSET] = salt.length;
    writeU32BE(fixed, MEMORY_KIB_OFFSET, params.memoryKib);
    writeU32BE(fixed, ITERATIONS_OFFSET, params.iterations);
    writeU32BE(fixed, PARALLELISM_OFFSET, params.parallelism);

    // Everything written so far — the parameters, the salt, and the nonce — is
    // the associated data, which is what makes a parameter downgrade detectable
    // rather than merely useless (KD-7).
    const associatedData = concatBytes(fixed, salt, nonce);
    const plaintext = new Uint8Array(PLAINTEXT_LENGTH);
    plaintext.set(request.masterKey.bytes(), 0);
    writeU64BE(plaintext, MASTER_KEY_LENGTH, request.epoch.value);

    const protectionKey = await deriveProtectionKey(params, request.passphrase, salt);
    const message = seal(protectionKey, nonce, associatedData, plaintext);
    return StoredMasterKey.fromBytes(concatBytes(associatedData, message));
  }

  /** Takes stored bytes, checking that they are this form at all. */
  static fromBytes(bytes: Uint8Array): StoredMasterKey {
    return new StoredMasterKey(Uint8Array.from(bytes), parseLayout(bytes));
  }

  /** The bytes to store, as a copy the caller owns. */
  bytes(): Uint8Array {
    return Uint8Array.from(this.#bytes);
  }

  /** The Argon2id cost these bytes were written at. */
  get params(): Argon2Params {
    return this.#layout.params;
  }

  /**
   * Opens the form with the Passphrase that protects it.
   *
   * The derivation follows the parameters recorded in these bytes rather than
   * this build's current policy, so a form written before a device raised its
   * cost still unlocks — and a form whose recorded cost was edited fails, as the
   * parameters are authenticated.
   */
  async unlock(passphrase: Uint8Array): Promise<UnlockedMasterKey> {
    const { params, saltEnd, nonceEnd, messageEnd } = this.#layout;
    const salt = this.#bytes.subarray(SALT_OFFSET, saltEnd);
    const nonce = this.#bytes.subarray(saltEnd, nonceEnd);

    const protectionKey = await deriveProtectionKey(params, passphrase, salt);
    const plaintext = open(
      protectionKey,
      nonce,
      this.#bytes.subarray(0, nonceEnd),
      this.#bytes.subarray(nonceEnd, messageEnd),
    );
    return {
      masterKey: MasterKey.fromBytes(plaintext.subarray(0, MASTER_KEY_LENGTH)),
      epoch: MasterKeyEpoch.of(readU64BE(plaintext, MASTER_KEY_LENGTH)),
    };
  }
}

/**
 * Reads the plaintext part of a stored form.
 *
 * A reader follows the recorded salt length rather than its own build's policy,
 * and rejects an unknown magic or version, a non-zero reserved byte, or a total
 * length that disagrees with the recorded salt length (KD-9).
 */
function parseLayout(bytes: Uint8Array): Layout {
  if (bytes.length < SALT_OFFSET) {
    fail(
      'stored_master_key_length_mismatch',
      `expected at least ${SALT_OFFSET} bytes, found ${bytes.length}`,
    );
  }
  if (!bytesEqual(bytes.subarray(0, STORED_MASTER_KEY_MAGIC.length), STORED_MASTER_KEY_MAGIC)) {
    fail('unknown_stored_master_key_magic', 'unknown magic, not a stored Master Key');
  }
  if (bytes[VERSION_OFFSET] !== STORED_MASTER_KEY_VERSION) {
    fail(
      'unsupported_stored_master_key_version',
      `unsupported stored Master Key version ${bytes[VERSION_OFFSET]}`,
    );
  }
  if (bytes[RESERVED_OFFSET] !== 0) {
    fail('reserved_not_zero', 'reserved header bytes are not zero');
  }
  const saltEnd = SALT_OFFSET + bytes[SALT_LENGTH_OFFSET];
  const nonceEnd = saltEnd + NONCE_LENGTH;
  const messageEnd = nonceEnd + PLAINTEXT_LENGTH + TAG_LENGTH;
  // The encrypted plaintext is a key and an epoch and nothing else, so the whole
  // form is exactly this long — no shorter, and with nothing appended.
  if (bytes.length !== messageEnd) {
    fail(
      'stored_master_key_length_mismatch',
      `the recorded salt length makes this form ${messageEnd} bytes, found ${bytes.length}`,
    );
  }
  return {
    params: {
      memoryKib: readU32BE(bytes, MEMORY_KIB_OFFSET),
      iterations: readU32BE(bytes, ITERATIONS_OFFSET),
      parallelism: readU32BE(bytes, PARALLELISM_OFFSET),
    },
    saltEnd,
    nonceEnd,
    messageEnd,
  };
}
