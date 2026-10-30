// What a drop onto a folder of the tree will do, said while the files are
// still in the air.
//
// Every drop is confirmed once it has been read and before anything is sent:
// where it lands, and what it is added as. So the pill at the bottom of the
// tree says only where the drop lands — `Drop to add to <path>` — with the
// row it lands in outlined, short enough to read at a glance over the rows.
//
// A folder no folder of this device is chosen for takes the drop all the same:
// the add flow asks for one first, inside the flow (spec: EP-9), so the drag
// has nothing to refuse there. What it does refuse is a path the Library holds
// nothing at — a component mistyped into the address bar — where nothing on
// this screen is a folder to add into.
//
// Kept free of DOM and of React so it is unit testable, as `unmapped.ts` beside
// it is.

import type { Listing } from '@coffret/api';

import { placeOf } from './destination';
import { NOTHING_AT_THIS_PATH } from './unmapped';

/**
 * What a drop onto a folder comes to.
 *
 * - `taken`: the drop is added, once it is confirmed.
 * - `taken_after`: the same, while a freeze is running already, so a folder
 *   added as a Pack now is packed after that one.
 * - `refused`: nothing is taken, because the Library holds nothing there.
 */
export type DropOutcome = 'taken' | 'taken_after' | 'refused';

/** What the screen knows about the folder a drag is over. */
export interface DropTarget {
  /**
   * Whether the folder is somewhere files can be added: a folder of the
   * Library, one prepared on this page, or one a freeze is packing — anything
   * but a path the Library holds nothing at.
   */
  held: boolean;
  /** Whether a book is being packed right now, wherever it is. */
  freezing: boolean;
}

/**
 * The outcome a drop onto the folder would come to.
 *
 * A folder added as a Pack while another freeze runs is still taken — the
 * server queues the second freeze behind the first, and they run one at a
 * time — so what changes is the order, and that is what is said.
 */
export function dropOutcome(target: DropTarget): DropOutcome {
  if (!target.held) {
    return 'refused';
  }
  return target.freezing ? 'taken_after' : 'taken';
}

/**
 * What a folder added as a Pack while another freeze runs comes to: freezes
 * run one at a time (spec: PK-7), so this one waits.
 */
export const PACKED_AFTER =
  'a book is being packed already, so a folder added as One Pack now is packed after it';

/** What letting go of files over `landing` does: the confirmation comes up for that folder. */
export function dropToAdd(landing: string): string {
  return `Drop to add to ${placeOf(landing)}`;
}

/**
 * The pill the tree shows while files are dragged over it, for the folder the
 * drop would land in.
 */
export function dropLine(outcome: DropOutcome, landing: string): string {
  switch (outcome) {
    case 'taken':
      return dropToAdd(landing);
    case 'taken_after':
      return `${dropToAdd(landing)} — ${PACKED_AFTER}`;
    case 'refused':
      return `a drop here is not taken — ${NOTHING_AT_THIS_PATH}`;
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
