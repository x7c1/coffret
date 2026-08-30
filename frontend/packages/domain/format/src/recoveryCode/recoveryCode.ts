/**
 * The form a Master Key takes on paper.
 *
 * The encoding is normative in KD-11; this module implements it. A Recovery
 * Code is the Master Key and its epoch in one Bech32m string, which is what a
 * user prints at Library creation and types into the next device they add.
 *
 * Bech32m is here for the transcription rather than for the cryptography: its
 * alphabet leaves out the four characters people confuse on paper (`1`, `b`,
 * `i`, `o`), and its checksum catches the substitutions a hand copy makes, so a
 * mistyped code is refused rather than read as a different key.
 *
 * Nothing here is Passphrase-derived and nothing here reaches Storage (KD-8).
 * This module deals in one string; printing it, and reading one a user typed,
 * belong to the layer that talks to a person.
 */

import { fail } from '../errors.js';
import { readU64BE, writeU64BE } from '../internal/bytes.js';
import { MASTER_KEY_LENGTH, MasterKey } from '../model/masterKey.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import {
  CHECKSUM_LENGTH,
  SEPARATOR,
  decodeBech32m,
  encodeBech32m,
  isFault,
  toBytes,
  type Bech32mFault,
} from './bech32m.js';

/** The human-readable part every Recovery Code starts with. */
export const RECOVERY_CODE_PREFIX = 'coffret';

/** The Recovery Code version this package writes and reads. */
export const RECOVERY_CODE_VERSION = 0x01;

/** Length of the payload the string carries, in bytes. */
export const RECOVERY_CODE_PAYLOAD_LENGTH = 1 + 8 + MASTER_KEY_LENGTH;

/**
 * How many characters the payload takes once regrouped into five-bit units:
 * 41 bytes are 328 bits, which fill 66 characters and leave two padding bits.
 */
export const RECOVERY_CODE_DATA_LENGTH = 66;

/** Length of the whole lowercase string: `coffret1`, the data, the checksum. */
export const RECOVERY_CODE_LENGTH =
  RECOVERY_CODE_PREFIX.length + 1 + RECOVERY_CODE_DATA_LENGTH + CHECKSUM_LENGTH;

/** How many characters one printed group holds. */
export const RECOVERY_CODE_GROUP_LENGTH = 4;

/** Which bits of the last data character lie past the 41st byte. */
const PADDING_BITS = 0b11;

const VERSION_OFFSET = 0;
const EPOCH_OFFSET = 1;
const MASTER_KEY_OFFSET = 9;

/** What a Recovery Code carries: a Master Key, and the epoch it belongs to. */
export interface RecoveryCodeContent {
  /** The Master Key the code backs up. */
  masterKey: MasterKey;
  /** The epoch that key belongs to. */
  epoch: MasterKeyEpoch;
}

/**
 * Writes a Master Key and its epoch as the code their owner keeps.
 *
 * The epoch travels with the key because a key alone does not say which control
 * objects on Storage it opens (KD-11).
 */
export function encodeRecoveryCode(content: RecoveryCodeContent): string {
  const payload = new Uint8Array(RECOVERY_CODE_PAYLOAD_LENGTH);
  payload[VERSION_OFFSET] = RECOVERY_CODE_VERSION;
  writeU64BE(payload, EPOCH_OFFSET, content.epoch.value);
  payload.set(content.masterKey.bytes(), MASTER_KEY_OFFSET);
  return encodeBech32m(RECOVERY_CODE_PREFIX, payload);
}

/**
 * Reads a code a user wrote down, or refuses to read it at all.
 *
 * Whitespace and hyphens go first, so the grouped printing form and any other
 * way the user broke the string up read the same as the bare one; an entirely
 * uppercase copy reads too, since Bech32 admits either case but not a mixture.
 *
 * Every remaining check either passes or throws naming itself, and none of them
 * releases key material: a code with a mistyped character yields no Master Key
 * rather than a different one (KD-11).
 */
export function decodeRecoveryCode(text: string): RecoveryCodeContent {
  const decoded = decodeBech32m(normalize(text));
  if (isFault(decoded)) {
    failFault(decoded);
  }

  if (decoded.prefix !== RECOVERY_CODE_PREFIX) {
    fail(
      'unknown_recovery_code_prefix',
      `unknown prefix ${JSON.stringify(decoded.prefix)}, not the ` +
        `${JSON.stringify(RECOVERY_CODE_PREFIX)} a Recovery Code starts with`,
    );
  }
  // The character count is the check rather than the byte count: 66 characters
  // and 67 both yield 41 bytes, and only the first of them is this form.
  if (decoded.data.length !== RECOVERY_CODE_DATA_LENGTH) {
    fail(
      'recovery_code_length_mismatch',
      `expected ${RECOVERY_CODE_DATA_LENGTH} data characters in a Recovery Code, found ${decoded.data.length}`,
    );
  }
  if ((decoded.data[RECOVERY_CODE_DATA_LENGTH - 1] & PADDING_BITS) !== 0) {
    fail('non_zero_recovery_code_padding', "a Recovery Code's padding bits are not zero");
  }

  const payload = toBytes(decoded.data);
  if (payload[VERSION_OFFSET] !== RECOVERY_CODE_VERSION) {
    fail(
      'unsupported_recovery_code_version',
      `unsupported Recovery Code version ${payload[VERSION_OFFSET]}`,
    );
  }
  return {
    masterKey: MasterKey.fromBytes(payload.subarray(MASTER_KEY_OFFSET)),
    epoch: MasterKeyEpoch.of(readU64BE(payload, EPOCH_OFFSET)),
  };
}

/**
 * The code as it is printed: everything after `coffret1` in groups of four, the
 * data characters and the checksum alike.
 *
 * Grouping is presentation and not part of the form — {@link decodeRecoveryCode}
 * strips it along with any other whitespace — so a code printed this way and the
 * same code typed back as one run of characters are one value (KD-11).
 */
export function groupRecoveryCode(code: string): string {
  const prefix = `${RECOVERY_CODE_PREFIX}${SEPARATOR}`;
  if (!code.startsWith(prefix)) {
    fail('malformed_recovery_code', 'this is not a Recovery Code');
  }
  const data = code.slice(prefix.length);

  const groups: string[] = [];
  for (let start = 0; start < data.length; start += RECOVERY_CODE_GROUP_LENGTH) {
    groups.push(data.slice(start, start + RECOVERY_CODE_GROUP_LENGTH));
  }
  return [prefix, ...groups].join(' ');
}

/** Drops what a person adds writing a code down by hand. */
function normalize(text: string): string {
  return text.replace(/[ \t\n\f\r-]/g, '');
}

/** Names the check the string failed before its payload was ever reached. */
function failFault(fault: Bech32mFault): never {
  switch (fault.fault) {
    case 'mixed_case':
      fail(
        'recovery_code_mixed_case',
        'a Recovery Code is written in one case, not a mixture of two',
      );
      break;
    case 'invalid_character':
      fail(
        'recovery_code_invalid_character',
        `a Recovery Code holds no character ${JSON.stringify(fault.character)}`,
      );
      break;
    case 'checksum':
      fail('recovery_code_checksum_failed', "a Recovery Code's checksum does not verify");
      break;
    // What is left is a string with no `1` to divide at, or a prefix that is
    // not characters a human-readable part may be built from: not a code with
    // something wrong in it, but not a code at all.
    case 'malformed':
      fail('malformed_recovery_code', 'this is not a Recovery Code');
  }
}
