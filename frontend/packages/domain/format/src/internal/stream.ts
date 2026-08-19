/**
 * Moving a Container's plaintext stream past the chunk boundary, one chunk at a
 * time.
 *
 * The stream is every Entry's plaintext back to back in entry-table order,
 * followed by the zero padding the meta section's `pad_len` records (FM-4).
 * Neither side of the format ever materializes that whole stream: the reader
 * fills one chunk buffer at a time, and the writer scatters one decrypted chunk
 * into the entries it overlaps.
 */

import { fail } from '../errors.js';
import type { EntrySource } from '../model/entry.js';

/** Fills chunk-sized buffers from the entries and the padding tail. */
export class StreamReader {
  readonly #entries: readonly EntrySource[];
  #entryIndex = 0;
  #entryOffset = 0;
  #paddingLeft: bigint;

  constructor(entries: readonly EntrySource[], padLength: bigint) {
    this.#entries = entries;
    this.#paddingLeft = padLength;
  }

  /**
   * Fills `buffer` from the stream, returning how many bytes were written.
   *
   * A short return means the stream is exhausted.
   */
  read(buffer: Uint8Array): number {
    let written = 0;
    while (written < buffer.length) {
      const entry = this.#entries[this.#entryIndex];
      if (entry !== undefined) {
        const remaining = entry.content.length - this.#entryOffset;
        if (remaining === 0) {
          this.#entryIndex += 1;
          this.#entryOffset = 0;
          continue;
        }
        const take = Math.min(remaining, buffer.length - written);
        buffer.set(entry.content.subarray(this.#entryOffset, this.#entryOffset + take), written);
        this.#entryOffset += take;
        written += take;
        continue;
      }
      if (this.#paddingLeft === 0n) {
        break;
      }
      const take = Math.min(Number(this.#paddingLeft), buffer.length - written);
      buffer.fill(0, written, written + take);
      this.#paddingLeft -= BigInt(take);
      written += take;
    }
    return written;
  }
}

/** Scatters decrypted chunks into one buffer per Entry. */
export class StreamWriter {
  readonly #contents: Uint8Array[];
  readonly #expected: bigint;
  #entryIndex = 0;
  #entryOffset = 0;
  #paddingLeft: bigint;
  #written = 0n;

  /**
   * Prepares buffers for entries of the given sizes, followed by `padLength`
   * bytes of padding.
   */
  constructor(sizes: readonly number[], padLength: bigint, expected: bigint) {
    this.#contents = sizes.map((size) => new Uint8Array(size));
    this.#paddingLeft = padLength;
    this.#expected = expected;
  }

  /** Appends one chunk's authenticated plaintext to the stream. */
  write(plaintext: Uint8Array): void {
    let offset = 0;
    while (offset < plaintext.length) {
      const content = this.#contents[this.#entryIndex];
      if (content !== undefined) {
        const left = content.length - this.#entryOffset;
        if (left === 0) {
          this.#entryIndex += 1;
          this.#entryOffset = 0;
          continue;
        }
        const take = Math.min(left, plaintext.length - offset);
        content.set(plaintext.subarray(offset, offset + take), this.#entryOffset);
        this.#entryOffset += take;
        offset += take;
        this.#written += BigInt(take);
        continue;
      }
      // The padding tail is discarded, but only up to `pad_len`: anything past
      // it means the stream is longer than the meta section says it is.
      const take = Math.min(Number(this.#paddingLeft), plaintext.length - offset);
      if (take === 0) {
        fail(
          'plaintext_length_mismatch',
          `expected ${this.#expected} plaintext bytes, decrypted more`,
        );
      }
      for (let index = offset; index < offset + take; index++) {
        if (plaintext[index] !== 0) {
          fail('non_zero_padding', 'padding tail is not zero-filled');
        }
      }
      this.#paddingLeft -= BigInt(take);
      offset += take;
      this.#written += BigInt(take);
    }
  }

  /** How many stream bytes have been written so far. */
  get written(): bigint {
    return this.#written;
  }

  /** The per-Entry plaintext buffers, in entry-table order. */
  contents(): Uint8Array[] {
    return this.#contents;
  }
}
