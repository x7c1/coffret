// What a gesture is answered with in a folder this device can do nothing in.
//
// The banner over such a folder is the standing reason it is there at all.
// These are the answers to the things that were just tried in front of it, and
// a gesture that changes nothing on the screen needs one: a row clicked and a
// drop let go of both look, a second later, exactly like a screen that was
// never touched.
//
// Two folders reach here and not one: a folder of the Library no mapping of
// this device reaches, and a path the Library holds nothing at. The gestures
// fail in the same place — nothing to fetch, nowhere to put it — so the answer
// is written here for both, and which reason it gives is the whole of what
// `held` decides.
//
// Kept free of DOM and of React so it is unit testable, the way the drop's own
// sentences beside it are.

import { NO_FOLDER_HERE } from '@coffret/api';

/** Which gesture met the folder. */
export type Tried = 'open' | 'add';

/**
 * Why a path the Library holds nothing at takes no gesture, as a clause a screen
 * sets inside a sentence of its own.
 *
 * One wording wherever it is said — the answer to a gesture here, the line a
 * drag brings up before it, and the line under rows that are not there — so
 * that one path never reads as two different facts.
 */
export const NOTHING_AT_THIS_PATH = 'the Library holds nothing at this path';

/**
 * The notice the Activity panel shows, naming what was tried and why it came to
 * nothing.
 *
 * Two sentences rather than one, differing in the half that says what was
 * tried. The reason is the same fact about the same folder, and it is worded the
 * same both times, so that reading the second after the first is reading one
 * new clause rather than a new sentence. What differs is everything a person
 * can check: they clicked a row, or they let go of files, and a line beginning
 * "nothing was added" under a row they clicked would have them looking for the
 * drop they did not make.
 *
 * Which fact it is, is `held`'s to say, and one path never gets both. A folder
 * the Library holds that no folder of this device is chosen for is the EP-9
 * reason: the folder is there, and this device has nowhere to put what is in
 * it. A path the Library holds nothing at is the other, and the missing folder
 * on this device is not what is wrong with it — the screen over these rows
 * deliberately offers no choice of folder for such a path, since being asked to
 * choose one for a part of the Library there is none of sends somebody choosing
 * a folder for nothing, and a notice that gave the EP-9 reason anyway would
 * leave one path carrying two explanations at once.
 *
 * An add into a folder with no folder chosen does not come here: the add flow
 * asks for one instead (see [`folderToChoose`](./mapping)), so what reaches
 * this for an add is a path the Library holds nothing at.
 */
export function unmappedLine(tried: Tried, held: boolean): string {
  if (!held) {
    // One clause for both gestures, because there is nothing further to say
    // about either: no folder chosen would change this. What was tried is the
    // only half that differs.
    return tried === 'open'
      ? `nothing was opened — ${NOTHING_AT_THIS_PATH}`
      : `nothing was added — ${NOTHING_AT_THIS_PATH}`;
  }
  // The server's own clause, since a click here asks it nothing.
  return tried === 'open'
    ? `nothing was opened — ${NO_FOLDER_HERE}, so there is nowhere here to put the file that row stands for`
    : `nothing was added — ${NO_FOLDER_HERE}`;
}
