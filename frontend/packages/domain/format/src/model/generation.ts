import { MAX_FORMAT_INTEGER } from '../internal/bytes.js';
import { fail } from '../errors.js';

/**
 * Where a control object sits in the Library's control history (FM-13).
 *
 * Journal records and activation Index Snapshots form one head chain, each
 * successor taking the head's generation plus 1; an ordinary Index Snapshot
 * takes the generation of the head it checkpoints; a Keyring counts its own
 * envelope sets. None of them restarts at a Master Key rotation, so an object
 * name is never reused across epochs, and the newest Journal record or Index
 * Snapshot is recognizable by name before any index exists.
 *
 * A generation is one of the integers the format bounds: it is at most
 * `MAX_FORMAT_INTEGER`, so a number past that names no generation (FM-19).
 */
export class Generation {
  readonly #value: bigint;

  private constructor(value: bigint) {
    this.#value = value;
  }

  /** The generation the Library's first head, and its first Keyring, is written as. */
  static readonly FIRST = new Generation(0n);

  /** Takes a generation number, or refuses one the format does not admit. */
  static of(value: bigint): Generation {
    if (value < 0n || value > MAX_FORMAT_INTEGER) {
      fail(
        'generation_out_of_range',
        `a generation is an unsigned number below 2^63, found ${value}`,
      );
    }
    return new Generation(value);
  }

  /** The generation number. */
  get value(): bigint {
    return this.#value;
  }

  /** The generation the successor of this head, or the next Keyring set, takes. */
  next(): Generation {
    if (this.#value >= MAX_FORMAT_INTEGER) {
      fail(
        'generation_out_of_range',
        'the last generation the format admits has no successor',
      );
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
