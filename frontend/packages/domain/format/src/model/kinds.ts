/**
 * Which kind of user-data Container this is.
 *
 * The kind is recorded explicitly rather than inferred from the entry count: a
 * Pack left holding a single Entry is still a Pack, and a replacement for a
 * one-file Container is still one-file. The spellings are the ones FM-9 gives
 * the meta section's `kind` field.
 */
export type ContainerKind = 'one-file' | 'pack';

/** Every Container kind, for callers that must cover them all. */
export const CONTAINER_KINDS: readonly ContainerKind[] = ['one-file', 'pack'];

/** Whether `value` is a Container kind this format version knows. */
export function isContainerKind(value: unknown): value is ContainerKind {
  return CONTAINER_KINDS.includes(value as ContainerKind);
}

/**
 * Which kind of control state a Storage Object carries.
 *
 * Control objects hold the Library's own bookkeeping — never user data, which
 * travels in Containers. Each kind is encrypted under its own purpose key
 * (KD-4), so a future kind arrives as a new variant together with a new info
 * string and a new kind byte (FM-11).
 *
 * The kind is what an object *is*, and it rides in the authenticated header.
 * What an object is *for* — a link in the control-head chain, a checkpoint, a
 * Keyring replica — is what its name says (FM-12), and the two are not the same
 * question: the head chain admits two kinds under one name form, because
 * whichever of them wins a head's commit slot takes that head's successor
 * position.
 *
 * `activation-snapshot` is the Index Snapshot that activates a new Master Key
 * epoch by winning a head's commit slot. It carries the same checkpoint content
 * an ordinary `index-snapshot` does, plus the fields activation needs, but it is
 * a kind of its own so that a misfiled or renamed object is refused on the
 * plaintext header and by the purpose key, before any payload is read.
 */
export type ControlObjectKind =
  | 'journal'
  | 'keyring'
  | 'index-snapshot'
  | 'activation-snapshot';

/** Every control-object kind, for callers that must cover them all. */
export const CONTROL_OBJECT_KINDS: readonly ControlObjectKind[] = [
  'journal',
  'keyring',
  'index-snapshot',
  'activation-snapshot',
];
