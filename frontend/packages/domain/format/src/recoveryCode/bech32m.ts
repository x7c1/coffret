/**
 * Bech32m (BIP-350), as much of it as a Recovery Code needs (KD-11).
 *
 * Written here rather than taken from a package: the point of the second
 * implementation is that it was written from the rule, and a shared dependency
 * would make the two sides agree by construction instead of by agreement. It is
 * also the whole of what the code form needs — no segwit witness versions, no
 * plain Bech32 — so the surface below is an encoder, a decoder, and the
 * five-bit regrouping between them.
 */

/** The 32 characters a data part is spelled in, in field-element order. */
const ALPHABET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';

/** How many characters the checksum takes. */
export const CHECKSUM_LENGTH = 6;

/** The character dividing the human-readable part from the data part. */
export const SEPARATOR = '1';

/** Bech32m's constant, the one value that distinguishes it from Bech32. */
const BECH32M_CONSTANT = 0x2bc830a3;

/** The generator of the BCH code the checksum comes from. */
const GENERATOR = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3];

const INVERSE_ALPHABET = new Map([...ALPHABET].map((character, value) => [character, value]));

/** A string taken apart into its two halves, before any payload check. */
export interface Hrpstring {
  /** The human-readable part, lowercased. */
  prefix: string;
  /** The data part as field elements, the checksum already removed. */
  data: number[];
}

/** Why a string is not a Bech32m string, in the terms KD-11's reader rejects in. */
export type Bech32mFault =
  | { fault: 'malformed' }
  | { fault: 'invalid_character'; character: string }
  | { fault: 'mixed_case' }
  | { fault: 'checksum' };

/** Spells a human-readable part and a payload as one lowercase Bech32m string. */
export function encodeBech32m(prefix: string, payload: Uint8Array): string {
  return encodeFieldElements(prefix, toFieldElements(payload));
}

/**
 * Spells data characters directly, checksum appended.
 *
 * The byte entry point above can only produce zero padding bits, since that is
 * what regrouping bytes leaves; this is the seam a caller that means to write
 * something else — a test of the padding rule — goes through.
 */
export function encodeFieldElements(prefix: string, data: readonly number[]): string {
  const checksum = createChecksum(prefix, data);
  return `${prefix}${SEPARATOR}${[...data, ...checksum].map(toCharacter).join('')}`;
}

/**
 * Takes a string apart, or says which check it failed.
 *
 * The order is the order KD-11 states: case, then alphabet and separator, then
 * the checksum. Nothing about the payload is looked at here — this layer knows
 * only that the string is a well-formed Bech32m string under some prefix.
 */
export function decodeBech32m(text: string): Hrpstring | Bech32mFault {
  if (mixesCase(text)) {
    return { fault: 'mixed_case' };
  }
  const lowered = text.toLowerCase();

  // The separator is the last one in the string, since a human-readable part
  // may hold the character and a data part may not.
  const separator = lowered.lastIndexOf(SEPARATOR);
  if (separator < 1) {
    return { fault: 'malformed' };
  }
  const prefix = lowered.slice(0, separator);
  for (const character of prefix) {
    const code = character.codePointAt(0) ?? 0;
    if (code < 33 || code > 126) {
      return { fault: 'malformed' };
    }
  }

  const elements: number[] = [];
  for (const character of lowered.slice(separator + 1)) {
    const value = INVERSE_ALPHABET.get(character);
    if (value === undefined) {
      return { fault: 'invalid_character', character };
    }
    elements.push(value);
  }
  if (
    elements.length < CHECKSUM_LENGTH ||
    polymod(expandPrefix(prefix).concat(elements)) !== BECH32M_CONSTANT
  ) {
    return { fault: 'checksum' };
  }
  return { prefix, data: elements.slice(0, elements.length - CHECKSUM_LENGTH) };
}

/** Whether a decoded string is one of the faults rather than a parsed one. */
export function isFault(result: Hrpstring | Bech32mFault): result is Bech32mFault {
  return 'fault' in result;
}

/**
 * Regroups bytes into five-bit field elements, padding the last one with zeros.
 *
 * The count is `ceil(bytes * 8 / 5)`, and whatever the last element holds past
 * the final byte is the padding a reader checks for zeros.
 */
export function toFieldElements(bytes: Uint8Array): number[] {
  const elements: number[] = [];
  let accumulator = 0;
  let bits = 0;
  for (const byte of bytes) {
    accumulator = (accumulator << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      elements.push((accumulator >> bits) & 0x1f);
    }
    // Only the bits still waiting for an element are kept, so the accumulator
    // stays inside the 32 bits the bitwise operators work in.
    accumulator &= (1 << bits) - 1;
  }
  if (bits > 0) {
    elements.push((accumulator << (5 - bits)) & 0x1f);
  }
  return elements;
}

/**
 * Regroups five-bit field elements back into whole bytes, dropping the tail.
 *
 * The leftover bits are dropped rather than checked here: whether they are zero
 * is a rule about the form being read, so the caller that knows the form makes
 * that check (KD-11).
 */
export function toBytes(elements: readonly number[]): Uint8Array {
  const bytes: number[] = [];
  let accumulator = 0;
  let bits = 0;
  for (const element of elements) {
    accumulator = (accumulator << 5) | element;
    bits += 5;
    while (bits >= 8) {
      bits -= 8;
      bytes.push((accumulator >> bits) & 0xff);
    }
    accumulator &= (1 << bits) - 1;
  }
  return Uint8Array.from(bytes);
}

function toCharacter(value: number): string {
  return ALPHABET[value];
}

/**
 * Whether a string mixes the two cases Bech32 lets a code be written in.
 *
 * ASCII only, because that is what the alphabet and the human-readable part are
 * made of: a character outside it is not a case question but an alphabet one,
 * and the alphabet check in {@link decodeBech32m} answers it.
 */
function mixesCase(text: string): boolean {
  return /[A-Z]/.test(text) && /[a-z]/.test(text);
}

/**
 * The checksum's view of the human-readable part: high bits, a zero, low bits.
 *
 * Spreading the prefix this way is what binds it to the data, so a code read
 * under the wrong prefix fails the checksum rather than decoding to a payload.
 */
function expandPrefix(prefix: string): number[] {
  const high: number[] = [];
  const low: number[] = [];
  for (const character of prefix) {
    const code = character.codePointAt(0) ?? 0;
    high.push(code >> 5);
    low.push(code & 0x1f);
  }
  return [...high, 0, ...low];
}

function createChecksum(prefix: string, data: readonly number[]): number[] {
  const values = expandPrefix(prefix).concat(data, [0, 0, 0, 0, 0, 0]);
  const residue = polymod(values) ^ BECH32M_CONSTANT;
  return Array.from({ length: CHECKSUM_LENGTH }, (_, index) => (residue >> (5 * (5 - index))) & 0x1f);
}

/** The BCH residue of a sequence of field elements. */
function polymod(values: readonly number[]): number {
  let checksum = 1;
  for (const value of values) {
    const top = checksum >> 25;
    checksum = ((checksum & 0x1ff_ffff) << 5) ^ value;
    for (const [index, generator] of GENERATOR.entries()) {
      if ((top >> index) & 1) {
        checksum ^= generator;
      }
    }
  }
  return checksum;
}
