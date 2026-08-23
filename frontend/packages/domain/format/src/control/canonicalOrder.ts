/**
 * The orders the arrays in a control-object payload are written in.
 *
 * Every array FM-15 and FM-16 define is in a stated order — Container IDs by
 * their sixteen bytes, Entries by the canonical UTF-8 bytes of their Entry Path
 * (EP-3) — so that one Library state has exactly one encoding. Two devices
 * committing the same batch produce the same map, and a record does not change
 * its bytes because a writer happened to hold its additions in a different
 * order.
 *
 * Putting an array in that order is the encoder's job, and checking it is the
 * decoder's. A decoder that sorted a payload into shape instead would accept two
 * encodings of one state and hide the writer that produced the second.
 */

import { fail } from '../errors.js';

/**
 * Rejects an array that is not strictly increasing under `compare`.
 *
 * Strictly, not merely non-decreasing: the keys these arrays are ordered by
 * identify their elements — one Container ID names one Container, and one Entry
 * Path holds at most one current Entry at a committed state (EP-5) — so a repeat
 * is a payload naming something twice, which the same check catches.
 */
export function requireStrictlyIncreasing<T>(
  array: string,
  items: readonly T[],
  compare: (left: T, right: T) => number,
): void {
  for (let index = 1; index < items.length; index += 1) {
    if (compare(items[index - 1], items[index]) >= 0) {
      fail(
        'control_payload_out_of_order',
        `element ${index} of ${array} does not follow its predecessor in the canonical order`,
      );
    }
  }
}

/** Compares two byte strings the way their raw bytes order. */
export function compareBytes(left: Uint8Array, right: Uint8Array): number {
  const shared = Math.min(left.length, right.length);
  for (let index = 0; index < shared; index += 1) {
    if (left[index] !== right[index]) {
      return left[index] < right[index] ? -1 : 1;
    }
  }
  return left.length - right.length;
}

/**
 * Compares two Entry Paths as EP-3 orders them: the canonical UTF-8 bytes,
 * independent of locale.
 *
 * Neither `localeCompare` nor `<` will do. The first is a runtime's collation
 * rather than the one every implementation computes. The second compares UTF-16
 * code units, which disagrees with UTF-8 byte order exactly where a path holds a
 * character above U+FFFF: that character's surrogates sort below U+E000 in
 * UTF-16 and above it in UTF-8, so an emoji and a private-use character would
 * land in one order here and the other order in the Rust implementation.
 *
 * Code points are compared instead, because UTF-8 encodes them in an order that
 * matches their numeric order — so this is UTF-8 byte order without building the
 * bytes, which the package has no encoder to do.
 */
export function comparePaths(left: string, right: string): number {
  const leftPoints = [...left];
  const rightPoints = [...right];
  const shared = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < shared; index += 1) {
    const one = leftPoints[index].codePointAt(0) ?? 0;
    const other = rightPoints[index].codePointAt(0) ?? 0;
    if (one !== other) {
      return one < other ? -1 : 1;
    }
  }
  return leftPoints.length - rightPoints.length;
}
