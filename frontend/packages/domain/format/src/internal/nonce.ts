/**
 * The nonces of coffret's AEAD messages.
 *
 * Inside a Container they are deterministic: `domain(1) ‖ counter(8,
 * big-endian) ‖ zero(15)` (FM-7). One Container Key encrypts exactly one
 * Container, so these never repeat under a key, and the counter plus the
 * separate final-chunk domain make reordering, truncation, and extension of the
 * chunk sequence fail authentication.
 *
 * The messages that are not part of a Container — control-object payloads, Key
 * Envelopes, a device's stored Master Key — have no such counter to hang a
 * domain off, and their keys each cover many messages, so they draw a random
 * nonce and carry it in the object.
 */

import { drawBytes } from './entropy.js';
import { writeU64BE } from './bytes.js';

/** Length of an XChaCha20-Poly1305 nonce in bytes. */
export const NONCE_LENGTH = 24;

/** The meta section. */
const DOMAIN_META = 0x01;
/** A chunk with more chunks after it. */
const DOMAIN_CHUNK = 0x02;
/** The last chunk of the stream. */
const DOMAIN_FINAL_CHUNK = 0x03;

/** The nonce of the meta section, whose counter is always zero. */
export function metaNonce(): Uint8Array {
  return buildNonce(DOMAIN_META, 0n);
}

/** The nonce of the chunk at `index`, counted from 0 across all chunks. */
export function chunkNonce(index: bigint, isFinal: boolean): Uint8Array {
  return buildNonce(isFinal ? DOMAIN_FINAL_CHUNK : DOMAIN_CHUNK, index);
}

/**
 * Draws a fresh nonce from the platform CSPRNG.
 *
 * 24 bytes is wide enough that random nonces never practically collide, which
 * is what lets one purpose key cover an unbounded number of objects.
 */
export function randomNonce(): Uint8Array {
  return drawBytes(NONCE_LENGTH);
}

function buildNonce(domain: number, counter: bigint): Uint8Array {
  const nonce = new Uint8Array(NONCE_LENGTH);
  nonce[0] = domain;
  writeU64BE(nonce, 1, counter);
  return nonce;
}
