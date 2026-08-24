import type { ContainerId } from './containerId.js';
import type { KeyEnvelope } from './keyEnvelope.js';

/**
 * What the committed Keyring records about one Container's key (KL-7).
 *
 * A current Container is mapped either to the envelope that opens it or to the
 * explicit key-lost marker; there is no third state and no absence of one, so a
 * Container is never silently unreadable.
 *
 * The marker is a statement about the committed control state alone. It makes
 * no claim about authenticated local key material, which may still restore an
 * envelope later (RV-8), and it does not take the Container out of the current
 * set (KL-17).
 */
export type ContainerKeyStatus =
  | { status: 'envelope'; envelope: KeyEnvelope }
  | { status: 'key-lost' };

/**
 * One Container the Keyring maps, and what it maps that Container to.
 *
 * The pair is the whole of what a Keyring records per Container: which
 * Container, and whether the committed control state holds its envelope or
 * records the key as lost (KL-7).
 */
export interface KeyringEntry {
  /** The Container this entry is about. */
  containerId: ContainerId;
  /** The key status the committed control state records for it. */
  key: ContainerKeyStatus;
}

/**
 * The complete mapping one Keyring generation carries (KL-6, KL-7).
 *
 * Every replica of a generation carries this same mapping, which is why reading
 * needs one valid replica and the replica count adds redundancy rather than a
 * quorum (KL-6). At every commit and `prune` boundary the committed mapping
 * covers every current Container and no other; whether a caller's mapping does
 * is the caller's obligation (KL-7).
 *
 * The order the entries are held in carries no meaning: the wire order is
 * Container ID order and the encoder puts them in it (FM-17), which is what
 * makes one mapping one byte string and therefore one `set_digest`, whichever
 * device wrote it (KL-1, KL-14).
 */
export interface KeyringMapping {
  /** The Containers this generation maps, in no order the caller has to keep. */
  entries: KeyringEntry[];
}
