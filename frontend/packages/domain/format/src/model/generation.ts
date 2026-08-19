import { U64_MAX } from '../internal/bytes.js';
import { fail } from '../errors.js';

/**
 * How many times a control object of one kind has been rewritten (FM-13).
 *
 * The generation counts that kind's own updates across the Library's whole life
 * and never restarts at a Master Key rotation, so a kind's object names are
 * never reused across epochs. It is what makes the newest Journal record or
 * Index Snapshot recognizable by name before any index exists.
 */
export class Generation {
  readonly #value: bigint;

  private constructor(value: bigint) {
    this.#value = value;
  }

  /** The generation the first object of a kind is written as. */
  static readonly FIRST = new Generation(0n);

  /** Takes a generation number. */
  static of(value: bigint): Generation {
    if (value < 0n || value > U64_MAX) {
      fail('generation_out_of_range', `a generation is an unsigned 64-bit number, found ${value}`);
    }
    return new Generation(value);
  }

  /** The generation number. */
  get value(): bigint {
    return this.#value;
  }

  /** The generation the next write of this object kind takes. */
  next(): Generation {
    if (this.#value >= U64_MAX) {
      fail('generation_out_of_range', 'the last representable generation has no successor');
    }
    return new Generation(this.#value + 1n);
  }

  /** Whether two values name the same generation. */
  equals(other: Generation): boolean {
    return this.#value === other.#value;
  }

  toString(): string {
    return this.#value.toString();
  }
}
