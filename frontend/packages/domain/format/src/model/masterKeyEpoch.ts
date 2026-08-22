import { U64_MAX } from '../internal/bytes.js';
import { fail } from '../errors.js';

/**
 * Which Master Key encrypted a piece of control state (FM-13).
 *
 * The Library's first epoch is 1, and each Master Key rotation increments it by
 * 1. The epoch is distinct from a control object's generation, which places the
 * object in the Library's control history.
 */
export class MasterKeyEpoch {
  readonly #value: bigint;

  private constructor(value: bigint) {
    this.#value = value;
  }

  /** The epoch a Library starts life in. */
  static readonly FIRST = new MasterKeyEpoch(1n);

  /** Takes an epoch number, which starts at 1. */
  static of(value: bigint): MasterKeyEpoch {
    if (value < 1n || value > U64_MAX) {
      fail('epoch_out_of_range', `Master Key epochs are numbered from 1 upward, found ${value}`);
    }
    return new MasterKeyEpoch(value);
  }

  /** The epoch number. */
  get value(): bigint {
    return this.#value;
  }

  /** The epoch a rotation from this one activates. */
  next(): MasterKeyEpoch {
    if (this.#value >= U64_MAX) {
      fail('epoch_out_of_range', 'the last representable epoch has no successor');
    }
    return new MasterKeyEpoch(this.#value + 1n);
  }

  /** Whether two values name the same epoch. */
  equals(other: MasterKeyEpoch): boolean {
    return this.#value === other.#value;
  }

  toString(): string {
    return this.#value.toString();
  }
}
