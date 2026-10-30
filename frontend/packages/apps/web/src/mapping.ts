// Choosing a folder on this device for part of the Library: from the banner
// over a folder no folder of this device is chosen for, and inside the add
// flow, where adding needs one and none is chosen yet.
//
// Underneath it is a mapping (spec: EP-9) — one top-level folder of the
// Library, or the whole of it, kept in a folder of this device — but that word
// is the command line's, and the person is never shown it: what they are asked
// is which folder of this device keeps those files, in the same terms the
// tree's "On this device" column speaks in.
//
// A page cannot be handed a real path on the device — a browser's own folder
// picker hands it files and never where they are — so the folders are browsed
// through the server (`browseFolders`) and the choice is recorded by it
// (`mapFolder`), which is `coffret map` without the terminal. What this page
// keeps is the picker's own state: the folder being shown, the path in the text
// field, and what refused the last request.
//
// Kept free of DOM and of React so it is unit testable, as `reconnect` is.

import type { Browsed, BrowsedFolder, Listing, Mapped } from '@coffret/api';

import { mappedAt } from './dropTarget';
import { said } from './useAsked';

/** What the banner's button says over a folder no folder of this device is chosen for. */
export const CHOOSE_A_FOLDER = 'Choose a folder…';

/** What opens the server-driven folder browser under the path field. */
export const BROWSE = 'Browse…';

/** What records the folder in the field and goes on. */
export const USE_THIS_FOLDER = 'Use this folder';

/** How the part of the Library a folder is chosen for is named: a top-level folder, or the Library. */
function named(prefix: string | null): string {
  return prefix === null ? 'the Library' : `“${prefix}”`;
}

/** What the step asks, as its heading. */
export function chooseTitle(prefix: string | null): string {
  return `Choose a folder on this device for ${named(prefix)}`;
}

/**
 * The line under the heading: what the folder is for, and how far it reaches.
 * A choice is keyed by one top-level folder of the Library, or by the whole of
 * it (spec: EP-9), so a folder chosen while adding to `comics/Foo` is the one
 * everything under `comics` uses — said here, since the person was adding to
 * one folder and is choosing for many.
 */
export function chooseLine(prefix: string | null): string {
  const reach = prefix === null ? 'everything in the Library' : `everything under ${named(prefix)}`;
  return `Files added are kept there as well as in the Library, and ${reach} uses it.`;
}

/**
 * The banner over a folder no folder of this device is chosen for: never that
 * the folder is "not on this device", which the tree's column says of a file
 * that is not here — a different fact, and one that holds of most rows of a
 * folder that has one chosen.
 */
export function noFolderChosenLine(prefix: string | null): string {
  return `No folder chosen for ${named(prefix)} yet`;
}

/**
 * Which folder of this device adding files to `target` has to be asked for
 * first, or `null` where none has to be: the add flow asks before the
 * confirmation, and only then (spec: EP-9).
 *
 * `root` is the Library root's listing, which names every top-level folder
 * this device has a folder for — the Library's folders by their own `mapped`,
 * and the ones with nothing in them yet by `empty_mapped_prefixes` — and says
 * by its own `mapped` whether one is chosen for the whole Library.
 *
 * - Into a folder: its top-level folder, unless a folder is chosen for it or
 *   for the whole Library.
 * - Into the Library root: the whole Library, unless a folder is chosen for
 *   it. The server takes an add at the root only through the whole Library's
 *   folder, even where every file would land under a top-level folder that
 *   has one of its own, so nothing less would let the add through.
 *
 * The prefix comes back as `null` for the whole Library.
 */
export function folderToChoose(target: string, root: Listing): { prefix: string | null } | null {
  if (root.mapped) {
    return null;
  }
  if (target === '') {
    return { prefix: null };
  }
  const top = target.split('/')[0];
  return mappedAt(top, root) === true ? null : { prefix: top };
}

/** One picker, open over the banner or the add flow it was opened from. */
export interface Picker {
  /**
   * The top-level folder of the Library a folder is chosen for, and `null`
   * for the whole Library.
   */
  prefix: string | null;
  /** The folder whose folders are listed, and `null` before the first answer. */
  browsed: Browsed | null;
  /**
   * The path in the text field: the browsed folder's, until somebody types
   * over it — and then what they typed, which is what *Use this folder* records.
   */
  typed: string;
  /** Whether a request is out, which is when the buttons wait for it. */
  busy: boolean;
  /** What refused the last request, and `null` where nothing did. */
  refused: string | null;
}

/** A picker as it opens, before anything has been listed. */
export function pickerFor(prefix: string | null): Picker {
  return { prefix, browsed: null, typed: '', busy: false, refused: null };
}

/** The picker with `text` in its text field, in place of the browsed path. */
export function typedOver(picker: Picker, text: string): Picker {
  return { ...picker, typed: text };
}

/** The folder *Use this folder* records: what is in the text field. */
export function chosen(picker: Picker): string {
  return picker.typed.trim();
}

/** Everything the picker reaches out to. */
export interface Asking {
  /** Lists a folder, `null` being the home directory: [`browseFolders`](@coffret/api). */
  browse: (path: string | null) => Promise<Browsed>;
  /** Records a mapping: [`mapFolder`](@coffret/api). */
  map: (localRoot: string, prefix: string | null) => Promise<Mapped>;
  /** Changes the picker, the way a state setter takes an update. */
  update: (change: (picker: Picker) => Picker) => void;
  /** The mapping was recorded, and this is what the server said of it. */
  mapped: (answer: Mapped) => void;
}

/**
 * Lists `path` — the home directory where it is `null` — and shows it.
 *
 * The listed folder's path replaces whatever was in the text field: browsing
 * is choosing, and *Use this folder* records the folder on the screen. It never rejects;
 * a refusal is the line in the picker, and the folder shown before stays.
 */
export async function browseTo(path: string | null, asking: Asking): Promise<void> {
  asking.update((picker) => ({ ...picker, busy: true, refused: null }));
  try {
    const answer = await asking.browse(path);
    asking.update((picker) => ({
      ...picker,
      busy: false,
      browsed: answer,
      typed: answer.path,
    }));
  } catch (refused: unknown) {
    asking.update((picker) => ({ ...picker, busy: false, refused: said(refused) }));
  }
}

/** Goes into one of the folders listed. */
export function descend(folder: BrowsedFolder, asking: Asking): Promise<void> {
  return browseTo(folder.path, asking);
}

/** Goes to the folder above the one listed, where there is one. */
export function toParent(picker: Picker, asking: Asking): Promise<void> {
  const parent = picker.browsed?.parent ?? null;
  return parent === null ? Promise.resolve() : browseTo(parent, asking);
}

/**
 * Records the folder in the text field for the picker's prefix.
 *
 * The prefix is the opener's and nothing here changes it. The answer is handed
 * on and the picker is done with; a refusal stays in the picker as its line,
 * and the picker stays open, because what a person does about one — choose
 * another folder, or correct what they typed — is done right there.
 */
export async function mapHere(picker: Picker, asking: Asking): Promise<void> {
  const localRoot = chosen(picker);
  if (localRoot === '') {
    asking.update((held) => ({
      ...held,
      refused: 'choose a folder, or type its whole path',
    }));
    return;
  }
  asking.update((held) => ({ ...held, busy: true, refused: null }));
  try {
    const answer = await asking.map(localRoot, picker.prefix);
    asking.update((held) => ({ ...held, busy: false }));
    asking.mapped(answer);
  } catch (refused: unknown) {
    asking.update((held) => ({ ...held, busy: false, refused: said(refused) }));
  }
}
