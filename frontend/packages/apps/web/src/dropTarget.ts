// What a drop onto a folder — the one the list is showing, or one a row of the
// tree names — will do, said while the files are still in the air.
//
// Every drop is confirmed once it has been read and before anything is sent:
// where it lands, and what it is added as. What it is added as follows what
// was dropped, not the folder it is dropped onto. Files on their own are added
// one at a time: the sync carries each one in as a Container of its own. A drop
// holding a folder is asked whether it goes in as a Pack, packed together in
// one batch (spec: PK-7), or as its files one by one. That is the
// confirmation's to say, with the drop read and counted in front of it, so the
// line while the drag is in the air says only that both are asked before
// anything goes — short enough to sit under a row of the narrow folder tree.
// It says the refusal there too, where a folder no mapping of this device
// reaches has nowhere to put any of them (spec: EP-9), and says nothing it
// cannot stand behind where whether a mapping reaches the folder is not known
// yet: a row of the tree is read off the root's listing, which may still be
// out when the drag begins or may not answer for that folder (see `mappedAt`).
//
// Kept free of DOM and of React so it is unit testable, as `unmapped.ts` beside
// it is.

import { NO_FOLDER_HERE, type Listing } from '@coffret/api';

import { NOTHING_AT_THIS_PATH } from './unmapped';

/**
 * What a drop onto a folder comes to.
 *
 * - `taken`: the drop is added, once it is confirmed.
 * - `taken_after`: the same, while a freeze is running already, so a folder
 *   added as a Pack now is packed after that one.
 * - `refused`: nothing is taken, because there is nowhere to put it.
 * - `unknown`: whether a mapping reaches the folder is not known yet. A drop
 *   let go then is not turned away here: the folder's own listing is asked
 *   before anything is sent, and that answer decides.
 */
export type DropOutcome = 'taken' | 'taken_after' | 'refused' | 'unknown';

/** What the screen knows about the folder a drag is over. */
export interface DropTarget {
  /**
   * Whether a mapping of this device reaches the folder, or `null` where that
   * is not known yet (see {@link mappedAt}).
   */
  mapped: boolean | null;
  /** Whether a book is being packed right now, wherever it is. */
  freezing: boolean;
}

/**
 * The outcome a drop onto the folder would come to.
 *
 * Read off the same fact the drop is turned away by, and nothing else: `mapped`
 * is what refuses a drop, so the line cannot promise one thing while the drop
 * does another.
 *
 * A folder added as a Pack while another freeze runs is still taken — the
 * server queues the second freeze behind the first, and they run one at a
 * time — so what changes is the order, and that is what is said.
 */
export function dropOutcome(target: DropTarget): DropOutcome {
  // Neither a promise nor a refusal: one said before the root's listing
  // answered would flip to the other for every row it was wrong about.
  if (target.mapped === null) {
    return 'unknown';
  }
  if (!target.mapped) {
    return 'refused';
  }
  return target.freezing ? 'taken_after' : 'taken';
}

/**
 * What a folder added as a Pack while another freeze runs comes to: freezes
 * run one at a time (spec: PK-7), so this one waits.
 */
export const PACKED_AFTER =
  'a book is being packed already, and books are packed in turn, so a folder added as a ' +
  'Pack now is packed after it';

/** What letting go of files over a folder that takes them does. */
export const TAKEN =
  'drop to add — where it goes, and how, is confirmed before anything is sent';

/** What a drag over a folder is told while whether it takes a drop is not known yet. */
export const NOT_KNOWN_YET =
  'whether a drop here is taken is not known yet — letting go asks, and nothing is sent ' +
  'before it is answered';

/**
 * The line the list shows while files are dragged over it.
 *
 * `held` is whether the Library holds the folder at all, and decides only the
 * refusal's reason — the same reason the answer to letting go gives, so the
 * line before the drop and the notice after it are one fact said twice.
 */
export function dropLine(outcome: DropOutcome, held: boolean): string {
  switch (outcome) {
    case 'taken':
      return TAKEN;
    case 'taken_after':
      return `${TAKEN} — ${PACKED_AFTER}`;
    case 'refused':
      return `a drop here is not taken — ${held ? NO_FOLDER_HERE : NOTHING_AT_THIS_PATH}`;
    case 'unknown':
      return NOT_KNOWN_YET;
  }
}

/**
 * Whether a mapping of this device reaches `folder`, as far as the listing of
 * the Library root says — or `null` where it does not say.
 *
 * A mapping stands for one top-level folder and everything under it, or for
 * the Library root (spec: EP-9), so the root's listing answers for every folder
 * the tree draws: the root by its own `mapped`, and any other folder by the
 * top-level folder it is under. A top-level folder not among the root's
 * folders is reached where the root's `empty_mapped_prefixes` names it. Any
 * other — the folder of a book whose freeze has not committed, a path prepared
 * on the page — is reached where the root is, and otherwise not known from
 * here: the drop asks its own listing before anything is sent.
 *
 * Read while a drag is over a row of the tree, which has no listing of its own
 * to ask and cannot wait for one.
 */
export function mappedAt(folder: string, root: Listing | null): boolean | null {
  if (root === null || root.path !== '') {
    return null;
  }
  if (folder === '') {
    return root.mapped;
  }
  const top = folder.split('/')[0];
  const listed = root.folders.find((one) => one.name === top);
  if (listed !== undefined) {
    return listed.mapped;
  }
  if (root.empty_mapped_prefixes.includes(top)) {
    return true;
  }
  return root.mapped ? true : null;
}
