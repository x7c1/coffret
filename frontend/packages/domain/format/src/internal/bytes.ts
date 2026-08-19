/**
 * Byte handling shared by every part of the format.
 *
 * Multi-byte integers are big-endian throughout coffret, and the 64-bit ones
 * are `bigint` here: a Container's offsets, sizes, generations, and epochs are
 * `u64` on the wire, and a JS `number` cannot carry all of them.
 */

import { fail } from '../errors.js';

/** Largest value a `u64` field can carry. */
export const U64_MAX = 0xffff_ffff_ffff_ffffn;

/** Largest value a `u32` field can carry. */
export const U32_MAX = 0xffff_ffff;

/** Largest value a `u16` field can carry. */
export const U16_MAX = 0xffff;

/** Joins byte strings into one. */
export function concatBytes(...parts: readonly Uint8Array[]): Uint8Array {
  let length = 0;
  for (const part of parts) {
    length += part.length;
  }
  const joined = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    joined.set(part, offset);
    offset += part.length;
  }
  return joined;
}

/** Whether two byte strings hold the same bytes. */
export function bytesEqual(left: Uint8Array, right: Uint8Array): boolean {
  if (left.length !== right.length) {
    return false;
  }
  for (let index = 0; index < left.length; index++) {
    if (left[index] !== right[index]) {
      return false;
    }
  }
  return true;
}

/** Whether every byte of `bytes` is zero. */
export function isAllZero(bytes: Uint8Array): boolean {
  for (const byte of bytes) {
    if (byte !== 0) {
      return false;
    }
  }
  return true;
}

/** The ASCII bytes of `text`, which must hold no character above U+007F. */
export function asciiBytes(text: string): Uint8Array {
  const bytes = new Uint8Array(text.length);
  for (let index = 0; index < text.length; index++) {
    const code = text.charCodeAt(index);
    if (code > 0x7f) {
      fail('value_out_of_range', `${JSON.stringify(text)} is not ASCII`);
    }
    bytes[index] = code;
  }
  return bytes;
}

const HEX_DIGITS = '0123456789abcdef';

/** The lowercase hex spelling of `bytes`. */
export function toHex(bytes: Uint8Array): string {
  let hex = '';
  for (const byte of bytes) {
    hex += HEX_DIGITS[byte >> 4] + HEX_DIGITS[byte & 0x0f];
  }
  return hex;
}

/**
 * Reads a lowercase hex string of `expectedLength` characters.
 *
 * Uppercase is rejected as well: identifiers are canonically lowercase, so
 * accepting both cases would let two spellings name the same value.
 */
export function fromHex(hex: string, expectedLength: number): Uint8Array {
  if (hex.length !== expectedLength) {
    fail(
      'invalid_hex_length',
      `expected ${expectedLength} hex characters, found ${hex.length}`,
    );
  }
  const bytes = new Uint8Array(hex.length / 2);
  for (let index = 0; index < bytes.length; index++) {
    const high = hexDigit(hex[index * 2]);
    const low = hexDigit(hex[index * 2 + 1]);
    bytes[index] = (high << 4) | low;
  }
  return bytes;
}

function hexDigit(character: string): number {
  const digit = HEX_DIGITS.indexOf(character);
  if (digit < 0) {
    fail('invalid_hex_digit', `expected a lowercase hex character, found ${JSON.stringify(character)}`);
  }
  return digit;
}

/** Copies a byte string that must be exactly `expectedLength` bytes long. */
export function takeExactly(bytes: Uint8Array, expectedLength: number, what: string): Uint8Array {
  if (bytes.length !== expectedLength) {
    fail(
      'invalid_byte_length',
      `${what} is ${expectedLength} bytes, found ${bytes.length}`,
    );
  }
  return Uint8Array.from(bytes);
}

function view(bytes: Uint8Array): DataView {
  return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

/** Reads a big-endian `u16`. */
export function readU16BE(bytes: Uint8Array, offset: number): number {
  return view(bytes).getUint16(offset, false);
}

/** Writes a big-endian `u16`. */
export function writeU16BE(bytes: Uint8Array, offset: number, value: number): void {
  view(bytes).setUint16(offset, value, false);
}

/** Reads a big-endian `u32`. */
export function readU32BE(bytes: Uint8Array, offset: number): number {
  return view(bytes).getUint32(offset, false);
}

/** Writes a big-endian `u32`. */
export function writeU32BE(bytes: Uint8Array, offset: number, value: number): void {
  view(bytes).setUint32(offset, value, false);
}

/** Reads a big-endian `u64`. */
export function readU64BE(bytes: Uint8Array, offset: number): bigint {
  return view(bytes).getBigUint64(offset, false);
}

/** Writes a big-endian `u64`. */
export function writeU64BE(bytes: Uint8Array, offset: number, value: bigint): void {
  view(bytes).setBigUint64(offset, value, false);
}

/**
 * Narrows a wire length to a `number` this runtime can allocate.
 *
 * A well-formed object written by a 64-bit implementation can still declare
 * lengths beyond what a JS array can hold; that is a limit of this reader, and
 * it says so rather than silently truncating.
 */
export function toLength(value: bigint, what: string): number {
  if (value < 0n || value > BigInt(Number.MAX_SAFE_INTEGER)) {
    fail('value_out_of_range', `${what} is beyond what this reader can address: ${value}`);
  }
  return Number(value);
}
