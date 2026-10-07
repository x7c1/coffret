// Choosing a folder on this device to map part of the Library to, from the
// banner over a folder this device does not have.
//
// A page cannot be handed a real path on the device — a browser's own folder
// picker hands it files and never where they are — so the folders are browsed
// through the server (`browseFolders`) and the mapping is recorded by it
// (`mapFolder`), which is `coffret map` without the terminal. What this page
// keeps is the picker's own state: the folder being shown, the path in the text
// field, and what refused the last request.
//
// Kept free of DOM and of React so it is unit testable, as `reconnect` is.

import type { Browsed, BrowsedFolder, Mapped } from '@coffret/api';

import { said } from './useAsked';

/** What the banner's button says over a folder this device does not have. */
export const MAP_THIS_FOLDER = 'map this folder…';

/** What it says over the Library root. */
export const MAP_THE_ROOT = 'map the Library root…';

/** What the picker's one button says. */
export const MAP_HERE = 'map here';

/**
 * The banner's button over a folder, by the top-level folder a mapping of it
 * is for.
 *
 * A mapping is keyed by one top-level folder of the Library (spec: EP-9), so a
 * folder further down is mapped by mapping the one at the top — and a button
 * that said "this folder" there would be promising a mapping no device can
 * hold.
 */
export function mapLabel(path: string, top: string): string {
  return path === top ? MAP_THIS_FOLDER : `map ${top}…`;
}

/** What the picker says it is for. */
export function pickerTitle(prefix: string | null): string {
  return prefix === null
    ? 'choose the folder on this device that holds the Library root'
    : `choose the folder on this device that holds ${prefix}`;
}

/** One picker, open over the banner it was opened from. */
export interface Picker {
  /**
   * The top-level folder of the Library being mapped, carried from the banner,
   * and `null` for the Library root.
   */
  prefix: string | null;
  /** The folder whose folders are listed, and `null` before the first answer. */
  browsed: Browsed | null;
  /**
   * The path in the text field: the browsed folder's, until somebody types
   * over it — and then what they typed, which is what *map here* maps.
   */
  typed: string;
  /** Whether a request is out, which is when the buttons wait for it. */
  busy: boolean;
  /** What refused the last request, and `null` where nothing did. */
  refused: string | null;
}

/** A picker as the banner opens it, before anything has been listed. */
export function pickerFor(prefix: string | null): Picker {
  return { prefix, browsed: null, typed: '', busy: false, refused: null };
}

/** The picker with `text` in its text field, in place of the browsed path. */
export function typedOver(picker: Picker, text: string): Picker {
  return { ...picker, typed: text };
}

/** The folder *map here* maps: what is in the text field. */
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
 * is choosing, and *map here* maps the folder on the screen. It never rejects;
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
 * Maps the folder in the text field to the picker's prefix.
 *
 * The prefix is the banner's and nothing here changes it. The answer is handed
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
