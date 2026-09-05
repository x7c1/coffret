/**
 * The CBOR the format's maps travel in.
 *
 * coffret's schemas are CBOR maps with text keys, and the encoding they use is
 * not part of the format: a decoder accepts any valid CBOR spelling of a
 * schema, and this encoder makes its own valid choices (it writes definite
 * lengths and canonically ordered keys). Only the field names and value types
 * are normative, which is what lets two independent implementations exchange
 * objects without agreeing on a byte-for-byte serializer.
 *
 * Maps are decoded as `Map`, not as plain objects: a wire map may carry keys
 * that are not text, and `__proto__` is a key like any other.
 */

import { decodeFirst, encode as encodeCborValue } from 'cborg';

import { MAX_FORMAT_INTEGER } from './bytes.js';
import { fail, type CoffretErrorCode } from '../errors.js';

/** A decoded CBOR map, keyed by whatever the writer put there. */
export type CborMap = Map<unknown, unknown>;

/** Serializes one CBOR item. */
export function encodeCbor(value: unknown, code: CoffretErrorCode): Uint8Array {
  try {
    return encodeCborValue(value);
  } catch (cause) {
    fail(code, `could not encode CBOR: ${describe(cause)}`, { cause });
  }
}

/**
 * Reads the first CBOR item, returning it and the bytes that follow it.
 *
 * CBOR is self-delimiting, so a plaintext that is one item followed by padding
 * needs no length field to tell the two apart.
 */
export function decodeCborFirst(
  bytes: Uint8Array,
  code: CoffretErrorCode,
): [unknown, Uint8Array] {
  try {
    const [value, remainder] = decodeFirst(bytes, { useMaps: true });
    return [value, remainder];
  } catch (cause) {
    fail(code, `malformed CBOR: ${describe(cause)}`, { cause });
  }
}

/** Reads exactly one CBOR item, rejecting anything that follows it. */
export function decodeCborExact(bytes: Uint8Array, code: CoffretErrorCode): unknown {
  const [value, remainder] = decodeCborFirst(bytes, code);
  if (remainder.length > 0) {
    fail(code, `${remainder.length} bytes follow the CBOR item`);
  }
  return value;
}

/** Reads one CBOR map, rejecting any other item. */
export function asCborMap(value: unknown, code: CoffretErrorCode, what: string): CborMap {
  if (!(value instanceof Map)) {
    fail(code, `${what} is a CBOR map`);
  }
  return value;
}

/**
 * Reads a field a schema declares as an unsigned 64-bit integer.
 *
 * A writer may spell a small value as a CBOR integer that decodes to a JS
 * number, so both spellings are accepted and normalized to `bigint`.
 */
export function requiredUint(map: CborMap, key: string, code: CoffretErrorCode): bigint {
  return asUint(map.get(key), key, code);
}

/** Reads a field a schema declares as a signed 64-bit integer. */
export function requiredInt(map: CborMap, key: string, code: CoffretErrorCode): bigint {
  const value = map.get(key);
  if (typeof value === 'bigint') {
    return value;
  }
  if (typeof value === 'number' && Number.isSafeInteger(value)) {
    return BigInt(value);
  }
  return fail(code, `${key} is an integer, found ${describeValue(value)}`);
}

/**
 * Normalizes a CBOR integer that must be zero or above and below 2^63.
 *
 * FM-19: every unsigned integer the format carries is below 2^63, so the bound
 * belongs where a wire integer becomes a value rather than at each field that
 * happens to end up in a type refusing it later. Every payload and meta-section
 * integer this package reads passes through here.
 *
 * A number past the bound is named in the message — it is the format's own
 * arithmetic and says nothing about the Library's content — while a value of
 * another shape is only described, a text field's content not being this
 * layer's to quote.
 */
export function asUint(value: unknown, what: string, code: CoffretErrorCode): bigint {
  const integer =
    typeof value === 'bigint'
      ? value
      : typeof value === 'number' && Number.isSafeInteger(value)
        ? BigInt(value)
        : undefined;
  if (integer === undefined || integer < 0n) {
    fail(code, `${what} is an unsigned integer below 2^63, found ${describeValue(value)}`);
  }
  if (integer > MAX_FORMAT_INTEGER) {
    fail(code, `${what} is an unsigned integer below 2^63, found ${integer}`);
  }
  return integer;
}

/** Reads an optional field a schema declares as an unsigned 64-bit integer. */
export function optionalUint(
  map: CborMap,
  key: string,
  code: CoffretErrorCode,
): bigint | undefined {
  return map.get(key) === undefined ? undefined : asUint(map.get(key), key, code);
}

/** Reads an optional field a schema declares as a signed 64-bit integer. */
export function optionalInt(map: CborMap, key: string, code: CoffretErrorCode): bigint | undefined {
  return map.get(key) === undefined ? undefined : requiredInt(map, key, code);
}

/**
 * Reads an optional field a schema declares as a boolean.
 *
 * The one such field is FM-17's `key_lost`, whose presence is the marker. Its
 * value is read all the same: the schema spells the marker `true`, and a reader
 * that took any value there as a marker would accept two spellings of it.
 */
export function optionalBool(
  map: CborMap,
  key: string,
  code: CoffretErrorCode,
): boolean | undefined {
  const value = map.get(key);
  if (value === undefined) {
    return undefined;
  }
  if (typeof value !== 'boolean') {
    fail(code, `${key} is a boolean, found ${describeValue(value)}`);
  }
  return value;
}

/** Reads a field a schema declares as text. */
export function requiredText(map: CborMap, key: string, code: CoffretErrorCode): string {
  const value = map.get(key);
  if (typeof value !== 'string') {
    fail(code, `${key} is text, found ${describeValue(value)}`);
  }
  return value;
}

/** Reads an optional field a schema declares as text. */
export function optionalText(map: CborMap, key: string, code: CoffretErrorCode): string | undefined {
  return map.get(key) === undefined ? undefined : requiredText(map, key, code);
}

/** Reads a field a schema declares as a byte string. */
export function requiredBytes(map: CborMap, key: string, code: CoffretErrorCode): Uint8Array {
  const value = map.get(key);
  if (!(value instanceof Uint8Array)) {
    fail(code, `${key} is a byte string, found ${describeValue(value)}`);
  }
  return value;
}

/** Reads a field a schema declares as an array. */
export function requiredArray(map: CborMap, key: string, code: CoffretErrorCode): unknown[] {
  const value = map.get(key);
  if (!Array.isArray(value)) {
    fail(code, `${key} is an array, found ${describeValue(value)}`);
  }
  return value;
}

function describe(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

function describeValue(value: unknown): string {
  if (value === undefined) {
    return 'nothing';
  }
  if (value instanceof Uint8Array) {
    return 'a byte string';
  }
  if (value instanceof Map) {
    return 'a map';
  }
  return Array.isArray(value) ? 'an array' : typeof value;
}
