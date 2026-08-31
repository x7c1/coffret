// The screen's place in the Library, as the URL carries it. Kept free of DOM so
// it is unit testable.

/** Where the explorer is standing. */
export interface ViewState {
  /** The folder being listed; the Library root is the empty string. */
  folder: string;
  /** The Entry Path of the file open in the reader, or `null` for the list. */
  open: string | null;
}

/** The Library root, with nothing open. */
export const AT_ROOT: ViewState = { folder: '', open: null };

/**
 * The state one URL hash names.
 *
 * The hash and not a path, and no router library behind it: this screen has one
 * place to be — a folder, and at most one file open in it — and a fragment
 * carries that without the server having to answer for every URL the browser
 * might be reloaded at.
 *
 * Anything unreadable is the Library root. A hash is something a person can
 * type, and the answer to one that names nothing is the screen the explorer
 * opens at rather than an error about a URL.
 */
export function parseHash(hash: string): ViewState {
  const params = new URLSearchParams(hash.replace(/^#/, ''));
  const open = params.get('open');
  return {
    folder: params.get('path') ?? '',
    open: open === null || open === '' ? null : open,
  };
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
  }
  return `#${parts.join('&')}`;
}

function encoded(path: string): string {
  return encodeURIComponent(path).replaceAll('%2F', '/');
}
