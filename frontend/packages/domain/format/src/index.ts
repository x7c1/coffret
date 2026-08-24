/**
 * Format v1: the byte form of every coffret Storage Object, and the keys that
 * open them.
 *
 * A Container — the object user data travels in — is a 32-byte plaintext
 * header, an encrypted CBOR meta section, and a sequence of separately
 * encrypted chunks. All of it is XChaCha20-Poly1305 under one Container Key,
 * with deterministic nonces and the header bound in as associated data, so
 * reordering, truncating, extending, or editing any part of the object fails
 * authentication.
 *
 * A control object — a Journal record, a Keyring replica, an Index Snapshot
 * ordinary or epoch-activating — is a 44-byte plaintext header and one AEAD
 * message under the purpose key of its kind. The name it is stored under says
 * what it is for rather than what it is (FM-12), so the encoder is told the kind
 * outright.
 *
 * What rides inside that message is the kind's own schema:
 * [`encodeJournalRecord`] writes what a commit records (FM-15),
 * [`encodeIndexSnapshot`] writes the Index of a whole Library (FM-16), and
 * [`encodeKeyring`] writes the mapping every replica of a Keyring generation
 * carries (FM-17), each producing the [`ControlPayload`] the framing seals.
 * [`keyringSetDigest`] is the one value a payload does not carry: the digest a
 * replica's name and a commit's selection both name the mapping by.
 *
 * The keys come from one Master Key: [`PurposeKey`] derives a key per
 * [`Purpose`], [`wrapContainerKey`] wraps a Container Key into the envelope the
 * Keyring stores, and [`StoredMasterKey`] is the form a device keeps its Master
 * Key in under a Passphrase.
 *
 * The package does no I/O of any kind — no file, network, or DOM access: every
 * entry point takes and returns `Uint8Array` and plain data, so the same code
 * runs in a browser and in Node.
 *
 * This is a second, independent implementation of the published specification,
 * written from the rules rather than from the reference implementation: what
 * the two agree on is what the specification actually says.
 *
 * ```ts
 * const key = generateContainerKey();
 * const container = encodeContainer({
 *   containerId: generateContainerId(),
 *   kind: 'one-file',
 *   key,
 *   entries: [{ path: 'photos/spring.jpg', mtimeSeconds: 1_700_000_000n, content }],
 * });
 * const opened = decodeContainer(container.bytes, key);
 * ```
 */

export { CoffretFormatError, type CoffretErrorCode } from './errors.js';

export {
  CONTAINER_HEADER_LENGTH,
  CONTAINER_MAGIC,
  CONTAINER_VERSION,
  DEFAULT_CHUNK_SIZE,
  encodeContainerHeader,
  parseContainerHeader,
  requireChunkSize,
  type ContainerHeader,
} from './containerHeader.js';

export { encodeContainer, type ContainerEncodeRequest, type EncodedContainer } from './encodeContainer.js';
export { decodeContainer, type DecodedContainer } from './decodeContainer.js';
export { META_SCHEMA, decodeMeta, encodeMeta, plaintextLength, type Meta } from './meta.js';
export { paddedLength } from './padme.js';

export {
  PURPOSES,
  PURPOSE_INFO,
  PURPOSE_KEY_LENGTH,
  PurposeKey,
  purposeOfControlObject,
  type Purpose,
} from './purposeKey.js';

export { unwrapContainerKey, wrapContainerKey } from './keyEnvelope.js';

export {
  CONTROL_HEADER_LENGTH,
  CONTROL_MAGIC,
  CONTROL_VERSION,
  encodeControlHeader,
  parseControlHeader,
  type ControlHeader,
} from './control/header.js';
export {
  encodeControlObject,
  type ControlEncodeRequest,
  type EncodedControlObject,
} from './control/encode.js';
export { decodeControlObject, type DecodedControlObject } from './control/decode.js';
export {
  decodeControlPayload,
  emptyPayloadBody,
  encodeControlPayload,
  type ControlPayload,
} from './control/payload.js';
export {
  JOURNAL_RECORD_SCHEMA,
  decodeJournalRecord,
  encodeJournalRecord,
} from './control/journalRecord.js';
export {
  INDEX_SNAPSHOT_SCHEMA,
  decodeIndexSnapshot,
  encodeIndexSnapshot,
  indexSnapshotKind,
  type IndexSnapshotPayload,
  type SnapshotActivation,
} from './control/indexSnapshot.js';
export {
  KEYRING_SCHEMA,
  decodeKeyring,
  encodeKeyring,
  keyringSetDigest,
} from './control/keyring.js';
export {
  controlObjectNamesEqual,
  formatControlObjectName,
  headName,
  indexSnapshotName,
  keyringReplicaName,
  nameAdmitsKind,
  parseControlObjectName,
  successorName,
  type ControlObjectName,
  type ControlObjectRole,
} from './control/objectName.js';

export {
  INITIAL_ARGON2_PARAMS,
  requireArgon2Params,
  type Argon2Params,
} from './storedMasterKey/argon2Params.js';
export {
  STORED_MASTER_KEY_MAGIC,
  STORED_MASTER_KEY_SALT_LENGTH,
  STORED_MASTER_KEY_VERSION,
  StoredMasterKey,
  type StoredMasterKeyCreateRequest,
  type UnlockedMasterKey,
} from './storedMasterKey/storedMasterKey.js';

export {
  CONTAINER_ID_HEX_LENGTH,
  CONTAINER_ID_LENGTH,
  ContainerId,
  STORAGE_EXTENSION,
  generateContainerId,
} from './model/containerId.js';
export { CONTAINER_KEY_LENGTH, ContainerKey, generateContainerKey } from './model/containerKey.js';
export { MASTER_KEY_LENGTH, MasterKey, generateMasterKey } from './model/masterKey.js';
export { MasterKeyEpoch } from './model/masterKeyEpoch.js';
export { Generation } from './model/generation.js';
export { ReplicaPosition } from './model/replicaPosition.js';
export { KEY_ENVELOPE_LENGTH, KeyEnvelope } from './model/keyEnvelope.js';
export {
  CONTAINER_KINDS,
  CONTROL_OBJECT_KINDS,
  isContainerKind,
  type ContainerKind,
  type ControlObjectKind,
} from './model/kinds.js';
export {
  CONTENT_HASH_LENGTH,
  type DecodedEntry,
  type DerivedFrom,
  type EntryMetadata,
  type EntrySource,
} from './model/entry.js';
export type { ContainerSummary } from './model/containerSummary.js';
export type { EntryLocation } from './model/entryLocation.js';
export {
  requireKeyringCommitment,
  type IndexCheckpoint,
  type KeyringCommitment,
} from './model/indexCheckpoint.js';
export type { ContainerAddition, JournalRecord } from './model/journalRecord.js';
export type {
  ContainerKeyStatus,
  KeyringEntry,
  KeyringMapping,
} from './model/keyringMapping.js';
export type { SnapshotContent } from './model/snapshotContent.js';
