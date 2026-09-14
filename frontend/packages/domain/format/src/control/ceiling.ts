/**
 * How long a control object of each kind may be.
 *
 * That each kind carries a ceiling, what the three are, and why a reader holds a
 * declared length against one before anything is sized by it, is the register's
 * (FM-11). What is here is why each number is the one it is.
 *
 * They are format decisions and live beside the payload schemas they bound: what
 * a Keyring costs per Container and what a Snapshot costs per Entry are FM-17's
 * and FM-16's answers, not a transport's. Being format decisions, they are also
 * not this implementation's to choose — the same three numbers bound the other
 * implementation of FM-11, and an object one of them writes is one the other
 * takes because both hold to these and to nothing else.
 */

import { fail } from '../errors.js';
import { nameAdmitsKind, type ControlObjectName } from './objectName.js';
import { CONTROL_OBJECT_KINDS, type ControlObjectKind } from '../model/kinds.js';

/**
 * The longest Journal record this build reads or writes (FM-11, FM-15).
 *
 * A record carries one commit's additions, and an addition carries the whole
 * entry table of the Container it adds — which is what lets a device replay a
 * record without opening a Container (CP-11). So a record is sized by the batch
 * and not by the Library, and the largest batch is an initial import: one
 * `freeze` invocation over a whole folder tree.
 *
 * At the ~120 bytes an Entry costs in a catalog payload (the design budget
 * FM-16's schema is measured against, and FM-15 spells the same entry map),
 * 256 MiB is a single commit of roughly two million Entries. A batch that size
 * is already a run of many hours; one past it is not a batch this format was
 * shaped for.
 */
export const MAX_JOURNAL_RECORD_LENGTH = 256 * 1024 * 1024;

/**
 * The longest Index Snapshot this build reads or writes, ordinary or activation
 * (FM-11, FM-16).
 *
 * The Snapshot is the one payload that grows with the whole Library rather than
 * with a batch, and a device whose Index is older than the newest checkpoint
 * fetches one entire (CK-9). At the schema's 120-byte design budget per Entry,
 * 512 MiB is a Library of some four million Entries — for a photo and book
 * collection, a decade of it several times over.
 *
 * The ceiling is where the format's own shape gives out rather than where a
 * number looked round: a Library past it needs a checkpoint that can be read in
 * pieces, which is a change to FM-16 and not a larger constant. Raising this one
 * without that change would only move where the same memory is spent.
 */
export const MAX_INDEX_SNAPSHOT_LENGTH = 512 * 1024 * 1024;

/**
 * The longest Keyring replica this build reads or writes (FM-11, FM-17).
 *
 * A Keyring maps every current Container to an envelope or a key-lost marker, so
 * it grows with the Container count — Containers, not Entries, which is why its
 * ceiling is the lowest of the three. At the ~110 bytes per Container the schema
 * is measured against, 64 MiB maps some six hundred thousand Containers; at the
 * gigabyte-scale Pack the size target aims for (PK-5), that is a Library
 * measured in hundreds of terabytes.
 *
 * Every generation is stored R times over and rewritten whole at each rotation
 * (KL-8, MR-1), so this is also the one ceiling that bounds what a rotation
 * reads and writes repeatedly.
 */
export const MAX_KEYRING_LENGTH = 64 * 1024 * 1024;

/** The longest object of one kind, header and tag included (FM-11). */
export function maxControlObjectLength(kind: ControlObjectKind): number {
  switch (kind) {
    case 'journal':
      return MAX_JOURNAL_RECORD_LENGTH;
    case 'keyring':
      return MAX_KEYRING_LENGTH;
    // An activation Snapshot is a Snapshot with two fields more (FM-16), so one
    // envelope covers both kinds.
    case 'index-snapshot':
    case 'activation-snapshot':
      return MAX_INDEX_SNAPSHOT_LENGTH;
  }
}

/**
 * The longest object a name may lead to, before its kind is known.
 *
 * A reader asks this of the *name*, because that is all it has when it decides
 * how many bytes it is willing to take in: the kind rides in the header, and the
 * header is inside the answer. A name admits one kind or two (FM-12), and the
 * answer is the larger of what it admits — refusing on the name alone would
 * refuse a legitimate object of the other kind.
 *
 * Nothing inside this package calls it: the package does no I/O, so
 * `decodeControlObject` is handed bytes something else has already read, and a
 * refusal there saves none of the memory FM-11 is protecting. It is exported
 * for the caller that *does* fetch — ask Storage for at most this many bytes
 * for `name`, and abandon an object that turns out to be longer.
 */
export function maxControlObjectLengthAt(name: ControlObjectName): number {
  // Every name form in FM-12's table admits at least one kind, so the 0 below
  // is never the answer: a name that admitted none would lead to no object this
  // build opens, and nothing would be worth reading for it.
  return CONTROL_OBJECT_KINDS.filter((kind) => nameAdmitsKind(name, kind))
    .map(maxControlObjectLength)
    .reduce((larger, ceiling) => Math.max(larger, ceiling), 0);
}

/**
 * Insists that a control-object length is one its kind may be.
 *
 * Called with a length that has been *declared* — by Storage, or by the bytes in
 * hand — and never with one that has been authenticated, which is the whole
 * point: it is what a reader consults before spending anything on the claim, and
 * what a writer consults before laying out an object no reader would take.
 */
export function requireControlObjectLength(kind: ControlObjectKind, length: number): number {
  const ceiling = maxControlObjectLength(kind);
  if (length > ceiling) {
    fail(
      'control_object_too_long',
      `a control object of ${length} bytes exceeds the ${ceiling} bytes an object of kind ${kind} may be`,
    );
  }
  return length;
}
