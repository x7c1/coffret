import type { ContainerId } from './containerId.js';
import type { KeyEnvelope } from './keyEnvelope.js';

/**
 * What the committed Keyring records about one Container's key (spec: KL-7).
 *
 * A current Container is mapped either to the envelope that opens it or to the
 * explicit key-lost marker; there is no third state and no absence of one, so a
 * Container is never silently unreadable.
 *
 * The marker is a statement about the committed control state alone. It makes
 * no claim about authenticated local key material, which may still restore an
 * envelope later (spec: RV-8), and it does not take the Container out of the
 * current set (spec: KL-17).
 */
export type ContainerKeyStatus =
  | { status: 'envelope'; envelope: KeyEnvelope }
  | { status: 'key-lost' };

/**
 * One Container the Keyring maps, and what it maps that Container to.
 *
 * The element is the whole of what a Keyring records per Container: which
 * Container, and whether the committed control state holds its envelope or
 * records the key as lost (spec: KL-7).
 */
export interface KeyringElement {
  /** The Container this element is about. */
  containerId: ContainerId;
  /** The key status the committed control state records for it. */
  key: ContainerKeyStatus;
}

/**
 * The complete key table one Keyring generation carries (spec: KL-6, KL-7).
 *
 * Every replica of a generation carries this same key table, which is why
 * reading needs one valid replica and the replica count adds redundancy rather
 * than a quorum (spec: KL-6). At every commit and `prune` boundary the committed
 * key table covers every current Container and no other; whether a caller's key
 * table does is the caller's obligation (spec: KL-7).
 *
 * The order the elements are held in carries no meaning: the wire order is
 * Container ID order and the encoder puts them in it (spec: FM-17), which is
 * what makes one key table one byte string and therefore one `set_digest`,
 * whichever device wrote it (spec: KL-1, KL-14). The payload carries it as the
 * field `mapping`.
 */
export interface KeyTable {
  /** The Containers this generation maps, in no order the caller has to keep. */
  elements: KeyringElement[];
}
