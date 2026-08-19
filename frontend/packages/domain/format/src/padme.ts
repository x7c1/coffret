import { U64_MAX } from './internal/bytes.js';

/**
 * Rounds a plaintext stream length up to its Padmé bucket boundary (FM-4).
 *
 * Padmé (from the PURBs work) rounds an unpadded length `L` up to the next
 * multiple of `2^(E-S)`, where `E = floor(log2 L)` and `S = floor(log2 E) + 1`.
 * A stream short enough that `E - S <= 0` is stored unpadded. Overhead is
 * bounded at about 12% and is typically a few percent.
 *
 * Padding this way blunts fingerprinting of known content by its exact stored
 * size, which is one of the few things a storage provider can still observe
 * about an otherwise opaque object.
 *
 * Lengths so large that the bucket boundary above them is not representable in
 * a `u64` are returned unpadded; no real stream reaches that size.
 */
export function paddedLength(unpadded: bigint): bigint {
  // log2 is undefined at 0, and E would be 0 at 1 — both are below the regime
  // where padding applies.
  if (unpadded < 2n) {
    return unpadded;
  }
  const e = floorLog2(unpadded);
  const s = floorLog2(BigInt(e)) + 1;
  if (e <= s) {
    return unpadded;
  }
  const mask = (1n << BigInt(e - s)) - 1n;
  const rounded = (unpadded + mask) & ~mask;
  return rounded > U64_MAX ? unpadded : rounded;
}

/** `floor(log2(value))`, for `value > 0`. */
function floorLog2(value: bigint): number {
  return value.toString(2).length - 1;
}
