// Asking the Library what is new, and what the screen does with the answer.
//
// The explorer reads the Library out of this device's catalog, and the catalog
// holds what this device has replayed. Nothing brings it forward on its own —
// there is no polling of the remote head and there is deliberately not going to
// be one — so a device that has just joined shows an empty Library, and one
// another device has committed into shows the Library as it was. This is the
// gesture that ends that: one request, and then the two questions the screen
// answers with, asked again.
//
// Kept free of DOM and of React so it is unit testable: what it does is a
// sequence — say nothing, ask, say what came of it, ask the folder and the tree
// for themselves again — and every step of it is worth stating once.

import type { Refreshed } from '@coffret/api';

import { said } from './useRemote';

/** Everything one refresh reaches out to. */
export interface Refreshing {
  /** Asks the server, which is [`refreshCatalog`](@coffret/api). */
  ask: () => Promise<Refreshed>;
  /** Says what the refresh came to, and `null` clears what the last one said. */
  line: (line: string | null) => void;
  /** Says what refused it, and `null` clears what refused the last one. */
  trouble: (line: string | null) => void;
  /** Asks the folder tree and the open folder what they hold now. */
  reload: () => void;
}

/**
 * Asks what is new, and puts the answer on the screen.
 *
 * The reload is what makes the rows appear: the catalog is where a listing comes
 * from, so a refresh that advanced it has changed every folder's answer — and
 * the tree's, since a commit can add a folder that was not there. It is asked
 * for even where nothing was gained, which costs two local reads and covers the
 * case a count cannot state: a commit that only moved or removed Entries
 * advanced the catalog without adding to it.
 *
 * Nothing is asked again where the refresh was refused. A catch-up that stopped
 * may still have carried the catalog part of the way — the Journal is replayed
 * one record at a time (spec: CK-9) — but it stopped short of the head, and
 * drawing that under a sentence saying Storage did not answer would put a
 * Library on the screen that is neither the one this device had nor the one
 * there is. The press that succeeds resumes from where this one stopped, and
 * reloads then.
 *
 * It never rejects. What a refusal becomes is the sentence beside the control
 * that was pressed, which is the whole of what a person can do about it: press
 * it again once the Storage it names is back.
 */
export async function askWhatIsNew(refreshing: Refreshing): Promise<void> {
  refreshing.line(null);
  refreshing.trouble(null);
  try {
    const refreshed = await refreshing.ask();
    refreshing.line(refreshedLine(refreshed));
    refreshing.reload();
  } catch (refused: unknown) {
    refreshing.trouble(said(refused));
  }
}

/**
 * What one finished refresh says, as a line beside the control.
 *
 * "Up to date" is the answer a person presses this for most often and it has to
 * be said out loud: a control that does nothing visible when there is nothing to
 * find is one that reads as broken.
 *
 * Whether the catalog advanced and whether it gained anything are separate
 * questions, because a commit that only removed Entries is both a Library that
 * changed and a catalog that gained nothing — and calling that up to date would
 * tell somebody their screen is current at the moment a row leaves it.
 */
export function refreshedLine(refreshed: Refreshed): string {
  if (!refreshed.advanced) {
    return 'the Library is up to date';
  }
  if (refreshed.gained > 0) {
    return refreshed.gained === 1 ? '1 new file' : `${refreshed.gained} new files`;
  }
  if (refreshed.gained < 0) {
    const gone = -refreshed.gained;
    return gone === 1 ? '1 file has left the Library' : `${gone} files have left the Library`;
  }
  return 'the Library changed';
}
