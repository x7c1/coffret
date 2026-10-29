// The screen's place in the Library, as the URL carries it. Kept free of DOM so
// it is unit testable.

/** Where the explorer is standing. */
export interface ViewState {
  /** The folder being listed; the Library root is the empty string. */
  folder: string;
  /** The Entry Path of the file the reader's sheet is open on, or `null`. */
  open: string | null;
  /**
   * The Entry Path of the folder the reader's sheet is open on, showing its
   * cover; absent where the sheet is not open on a folder.
   *
   * Its own field rather than `open` again, since the Library admits a file
   * and a folder at one path (see [`folderKey`](./tree)), and a URL naming one
   * of them has to say which. Absent rather than `null` so that the many
   * places a state is written with no sheet on a folder need not spell it.
   */
  cover?: string;
}

/** The Library root, with nothing open. */
export const AT_ROOT: ViewState = { folder: '', open: null };

/**
 * The state one URL hash names.
 *
 * The hash and not a path, and no router library behind it: this screen has one
 * place to be — a folder, and at most one row open in the reader — and a fragment
 * carries that without the server having to answer for every URL the browser
 * might be reloaded at.
 *
 * Anything unreadable is the Library root. A hash is something a person can
 * type, and the answer to one that names nothing is the screen the explorer
 * opens at rather than an error about a URL.
 *
 * Every path comes back in NFC, because that is the form every Entry Path is in
 * (spec: EP-1) and every comparison made with the parsed state is against one.
 * A hash typed or pasted in NFD — which is how some systems spell `café` — would
 * otherwise name a folder and a file that match nothing the Library holds. The
 * address bar is left as it was typed: it names the same place, and the next
 * move the screen makes writes the hash in NFC anyway.
 */
export function parseHash(hash: string): ViewState {
  const params = new URLSearchParams(hash.replace(/^#/, ''));
  const open = params.get('open');
  const cover = params.get('cover');
  const state: ViewState = {
    folder: (params.get('path') ?? '').normalize('NFC'),
    open: open === null || open === '' ? null : open.normalize('NFC'),
  };
  // A file named as well wins: the sheet is open on one row at a time.
  if (state.open === null && cover !== null && cover !== '') {
    state.cover = cover.normalize('NFC');
  }
  return state;
}

/** Whether the reader's sheet is open: on a file, or on a folder's cover. */
export function sheetOpen(state: ViewState): boolean {
  return state.open !== null || state.cover !== undefined;
}

/**
 * The hash one state is carried by.
 *
 * `/` is left as itself rather than escaped. It is the only logical separator an
 * Entry Path has, so it can never be part of a name, and a hash that spelled it
 * `%2F` would make the address bar unreadable for no gain. Everything else goes
 * through `encodeURIComponent`, `&` and `=` included.
 */
export function toHash(state: ViewState): string {
  const parts: string[] = [];
  if (state.folder !== '') {
    parts.push(`path=${encoded(state.folder)}`);
  }
  if (state.open !== null) {
    parts.push(`open=${encoded(state.open)}`);
  } else if (state.cover !== undefined) {
    parts.push(`cover=${encoded(state.cover)}`);
  }
  return `#${parts.join('&')}`;
}

function encoded(path: string): string {
  return encodeURIComponent(path).replaceAll('%2F', '/');
}
