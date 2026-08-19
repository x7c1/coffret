import { U16_MAX } from '../internal/bytes.js';
import { fail } from '../errors.js';

/**
 * Which replica of a replicated control object this is, out of how many.
 *
 * Only Keyrings are replicated; a Journal record and an Index Snapshot are each
 * written once and therefore carry [`ReplicaPosition.SINGLE`] — replica index 0,
 * count 1 (FM-12). The count provides redundancy against individual object loss
 * and carries no quorum semantics.
 */
export class ReplicaPosition {
  readonly #index: number;
  readonly #count: number;

  private constructor(index: number, count: number) {
    this.#index = index;
    this.#count = count;
  }

  /** The position of an object that is written exactly once: replica 0 of 1. */
  static readonly SINGLE = new ReplicaPosition(0, 1);

  /**
   * Takes a 0-based replica index and the replica count it belongs to.
   *
   * A set has at least one replica, and every index names a replica the count
   * declares.
   */
  static of(index: number, count: number): ReplicaPosition {
    const valid =
      Number.isInteger(index) &&
      Number.isInteger(count) &&
      index >= 0 &&
      count >= 1 &&
      count <= U16_MAX &&
      index < count;
    if (!valid) {
      fail('invalid_replica_position', `replica ${index} is not one of ${count} replicas`);
    }
    return new ReplicaPosition(index, count);
  }

  /** The 0-based index of this replica. */
  get index(): number {
    return this.#index;
  }

  /** How many replicas the set declares. */
  get count(): number {
    return this.#count;
  }

  /** Whether two values name the same position in the same sized set. */
  equals(other: ReplicaPosition): boolean {
    return this.#index === other.#index && this.#count === other.#count;
  }

  toString(): string {
    return `r${this.#index}-of-${this.#count}`;
  }
}
