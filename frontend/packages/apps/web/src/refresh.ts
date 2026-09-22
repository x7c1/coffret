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

import type { Catalog, CatalogState, Refreshed } from '@coffret/api';

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

/**
 * Whether the answer just heard is a catch-up that has landed.
 *
 * The other way the catalog moves: not the gesture above, but a catch-up
 * somebody started elsewhere — another tab's press, a run this window only
 * watched — finishing while this screen was following it. Every listing comes
 * out of the catalog, so one that reached the Library's head has changed the
 * tree and the open folder at once, and a screen that did not ask again would
 * go on showing what the catalog held before.
 *
 * It is the promise the banner makes while the catch-up runs — the rest arrives
 * when it lands — kept for the person who waited rather than pressing anything.
 * Theirs is the reading that would otherwise end worst: the banner goes away by
 * itself, and what is left is the old rows with nothing on the screen still
 * saying they are not all of it.
 *
 * Read as a catalog that was not at the head and now is, rather than as the one
 * step from `catching_up`. A window that polls every so often can be told
 * `behind` and then `caught_up` with the run that fixed it having begun and
 * ended between two answers, and that is the same news arriving in fewer words.
 *
 * Two answers are not it. Coming up to a caught-up server — nothing before this
 * one — is a page that has just asked for the tree and the folder, and asking
 * again would be asking twice. And a catch-up that ended `behind` is not it
 * either, for the reason a refused refresh reloads nothing: it stopped short of
 * the head, and half of it under a sentence saying Storage did not answer is
 * neither the Library this device had nor the one there is.
 */
export function catchUpLanded(before: CatalogState | null, now: CatalogState): boolean {
  return before !== null && before !== 'caught_up' && now === 'caught_up';
}

/**
 * What is written on the control that asks the Library what is new.
 *
 * Named here rather than on the bar that draws it, because two places say it:
 * the button, and the sentence below that tells somebody to press the button.
 * Two literals would be a banner naming a control that no longer goes by that
 * name.
 */
export const ASKING = 'look for what is new';

/**
 * What the screen says about a catalog that is not the Library's, or `null`
 * where it is.
 *
 * The one sentence an empty explorer cannot say for itself. Every listing comes
 * out of this device's catalog, and the catalog holds what this device has
 * replayed — so a device fresh from `join` whose catch-up did not land shows
 * nothing, and shows it in exactly the way a Library holding nothing does. A
 * person cannot tell the two apart from the rows, and the difference is the
 * difference between "there is nothing here" and "this is not all of it".
 *
 * Both sentences end on what to do. Being told to wait and being told to press
 * the control that asks again are each better than being shown an empty Library
 * that is not empty.
 *
 * The second names that control by the words written on it. This sentence
 * stands at the top of the screen and the control is in the bar at the bottom,
 * so a person told to "ask what is new" would be left looking for a button of
 * that name among three others that all offer a second attempt.
 */
export function catalogLine(catalog: Catalog | null): string | null {
  if (catalog === null) {
    return null;
  }
  switch (catalog.state) {
    case 'caught_up':
      return null;
    case 'catching_up':
      return (
        'this device is catching up with the Library — what is listed is what it ' +
        'knew before, and the rest arrives when the catch-up lands'
      );
    case 'behind':
      return (
        'this device has not caught up with the Library, so what is listed may not be ' +
        `all of it — ${
          catalog.trouble?.message ?? 'the catch-up did not finish'
        }. Press "${ASKING}" to try again`
      );
  }
}
