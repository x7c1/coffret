import { describe, expect, it } from 'vitest';

import { errorCode } from '../errors.testing.js';
import { MasterKey } from '../model/masterKey.js';
import { MasterKeyEpoch } from '../model/masterKeyEpoch.js';
import { encodeBech32m, encodeFieldElements, toFieldElements } from './bech32m.js';
import {
  RECOVERY_CODE_DATA_LENGTH,
  RECOVERY_CODE_GROUP_LENGTH,
  RECOVERY_CODE_LENGTH,
  RECOVERY_CODE_PAYLOAD_LENGTH,
  RECOVERY_CODE_PREFIX,
  RECOVERY_CODE_VERSION,
  decodeRecoveryCode,
  encodeRecoveryCode,
  groupRecoveryCode,
} from './recoveryCode.js';

/**
 * A key whose every byte differs, so a reader that dropped or reordered bytes
 * lands somewhere else rather than on the same value.
 */
const MASTER_KEY = MasterKey.fromBytes(
  Uint8Array.from({ length: 32 }, (_, index) => (index * 31 + 7) & 0xff),
);

/** The payload KD-11 defines, as the rejections below start from before editing. */
function payload(version: number, epoch: bigint, key: MasterKey): Uint8Array {
  const bytes = new Uint8Array(RECOVERY_CODE_PAYLOAD_LENGTH);
  bytes[0] = version;
  new DataView(bytes.buffer).setBigUint64(1, epoch, false);
  bytes.set(key.bytes(), 9);
  return bytes;
}

describe('the Recovery Code', () => {
  // KD-11: a code carries the Master Key and the epoch, and reading one back
  // gives exactly the pair that was written.
  it('round-trips the key and the epoch', () => {
    for (const epoch of [1n, 0xffff_ffff_ffff_ffffn]) {
      const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.of(epoch) });
      const decoded = decodeRecoveryCode(code);

      expect(decoded.masterKey.bytes()).toEqual(MASTER_KEY.bytes());
      expect(decoded.epoch.value).toBe(epoch);
    }
  });

  // KD-11: `coffret1`, 66 data characters and a 6-character checksum, lowercase.
  it('is eighty lowercase characters under the coffret prefix', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.FIRST });

    expect(code).toHaveLength(RECOVERY_CODE_LENGTH);
    expect(code).toHaveLength(80);
    expect(code).toBe(code.toLowerCase());
    expect(code.startsWith(`${RECOVERY_CODE_PREFIX}1`)).toBe(true);
  });

  // KD-11: the printed grouping is presentation, so it reads back as the same
  // code — and it is everything after `coffret1` that is grouped, not the prefix.
  it('reads the grouped printing form back as the same code', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.of(42n) });
    const grouped = groupRecoveryCode(code);

    const [prefix, ...groups] = grouped.split(' ');
    expect(prefix).toBe(`${RECOVERY_CODE_PREFIX}1`);
    // The checksum is grouped along with the data characters, so the 72
    // characters after `coffret1` are 18 full groups and no short one.
    expect(groups).toHaveLength(18);
    for (const group of groups) {
      expect(group).toHaveLength(RECOVERY_CODE_GROUP_LENGTH);
    }

    const decoded = decodeRecoveryCode(grouped);
    expect(decoded.masterKey.bytes()).toEqual(MASTER_KEY.bytes());
    expect(decoded.epoch.value).toBe(42n);
  });

  // KD-11: whitespace and hyphens go before anything else, so however the user
  // broke the string up on paper, the code is the code.
  it('strips whitespace and hyphens', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.of(9n) });
    const broken = `  ${code.slice(0, 20)}-${code.slice(20, 50)}\n\t${code.slice(50)}  `;

    expect(decodeRecoveryCode(broken).epoch.value).toBe(9n);
  });

  // KD-11: Bech32 admits a code written entirely in either case.
  it('reads an uppercase copy', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.of(2n) });
    const decoded = decodeRecoveryCode(code.toUpperCase());

    expect(decoded.masterKey.bytes()).toEqual(MASTER_KEY.bytes());
    expect(decoded.epoch.value).toBe(2n);
  });

  // KD-11: a mixture of cases is not a third spelling of the code — no checksum
  // can be verified over it.
  it('refuses a mixed-case copy', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.FIRST });
    const mixed = `${code.slice(0, 40).toUpperCase()}${code.slice(40)}`;

    expect(errorCode(() => decodeRecoveryCode(mixed))).toBe('recovery_code_mixed_case');
  });

  // KD-11: the checksum is what makes a hand copy safe — one wrong character
  // ends the read rather than yielding a different Master Key.
  it('refuses a flipped character', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.FIRST });
    const at = 30;
    const replacement = code[at] === 'q' ? 'p' : 'q';
    const flipped = `${code.slice(0, at)}${replacement}${code.slice(at + 1)}`;

    expect(errorCode(() => decodeRecoveryCode(flipped))).toBe('recovery_code_checksum_failed');
  });

  // KD-11: a character outside the Bech32 alphabet is a transcription mistake,
  // and the four the alphabet leaves out are the ones people make.
  it('refuses a character outside the alphabet', () => {
    const code = encodeRecoveryCode({ masterKey: MASTER_KEY, epoch: MasterKeyEpoch.FIRST });
    const typo = `${code.slice(0, 30)}b${code.slice(31)}`;

    expect(errorCode(() => decodeRecoveryCode(typo))).toBe('recovery_code_invalid_character');
  });

  // KD-11: a well-formed Bech32m string under someone else's prefix is not a
  // Recovery Code, however sound its checksum.
  it('refuses another prefix', () => {
    const text = encodeBech32m('wrong', payload(RECOVERY_CODE_VERSION, 1n, MASTER_KEY));

    expect(errorCode(() => decodeRecoveryCode(text))).toBe('unknown_recovery_code_prefix');
  });

  // KD-11: the payload is 41 bytes exactly — 66 data characters — so a code
  // carrying one byte fewer or more is refused rather than read part-way.
  it('refuses a payload of another length', () => {
    for (const length of [40, 42]) {
      const text = encodeBech32m(RECOVERY_CODE_PREFIX, new Uint8Array(length).fill(0x11));

      expect(errorCode(() => decodeRecoveryCode(text))).toBe('recovery_code_length_mismatch');
    }
  });

  // KD-11: the two bits left over past the 41st byte are zero, so a writer that
  // put anything there wrote a string this form does not define.
  it('refuses non-zero padding bits', () => {
    const elements = toFieldElements(payload(RECOVERY_CODE_VERSION, 1n, MASTER_KEY));
    expect(elements).toHaveLength(RECOVERY_CODE_DATA_LENGTH);
    elements[elements.length - 1] |= 0b11;

    // The checksum has to cover the edited characters, or the read would end at
    // the checksum instead of at the padding.
    const text = encodeFieldElements(RECOVERY_CODE_PREFIX, elements);

    expect(errorCode(() => decodeRecoveryCode(text))).toBe('non_zero_recovery_code_padding');
  });

  // KD-11: the version byte leads the payload so a later form can change what
  // follows it; a build that does not know a version reads none of it.
  it('refuses an unknown version', () => {
    const text = encodeBech32m(RECOVERY_CODE_PREFIX, payload(0x02, 1n, MASTER_KEY));

    expect(errorCode(() => decodeRecoveryCode(text))).toBe('unsupported_recovery_code_version');
  });

  // KD-11: epochs are numbered from 1 (FM-13), so a code claiming epoch 0
  // carries no pair a Library could have written.
  it('refuses epoch zero', () => {
    const text = encodeBech32m(RECOVERY_CODE_PREFIX, payload(RECOVERY_CODE_VERSION, 0n, MASTER_KEY));

    expect(errorCode(() => decodeRecoveryCode(text))).toBe('epoch_out_of_range');
  });

  // KD-11: a string that divides into no prefix and data part is not a code with
  // something wrong in it — there is nothing to run any of the other checks over.
  it('refuses a string that divides into no prefix and data part', () => {
    for (const text of ['qqqqqqqq', '1qqqqqqq']) {
      expect(errorCode(() => decodeRecoveryCode(text))).toBe('malformed_recovery_code');
    }
  });

  // A code cut short after its separator still divides into a prefix and a data
  // part, so it is the checksum that ends the read rather than the refusal above
  // — which is also what the Rust implementation answers with.
  it('fails the checksum on a code cut short after the separator', () => {
    for (const text of ['coffret1', 'coffret1qqq']) {
      expect(errorCode(() => decodeRecoveryCode(text))).toBe('recovery_code_checksum_failed');
    }
  });
});
