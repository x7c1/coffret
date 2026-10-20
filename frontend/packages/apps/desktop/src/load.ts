// Loading a window's page afresh from a URL that carries a credential in its
// fragment.

/** What a window's contents offer for a page to be loaded into them. */
export interface Loadable {
  loadURL(url: string): Promise<void>;
  navigationHistory: { clear(): void };
}

/** The empty page a window passes through on its way to a fresh load. */
export const BLANK = 'about:blank';

/**
 * Loads `url` into `contents` as a new document, by way of an empty page.
 *
 * Both the door and the explorer take the token or ticket out of the address
 * as their page loads, which leaves the window at the bare origin. A URL that
 * differs from that only in its fragment would be a navigation within the
 * same document: the page would not load again, and would never read the new
 * fragment. Passing through an empty page first makes it a load of its own.
 * The window's history is cleared afterwards, so that going back does not
 * reach the empty page or the page before it.
 */
export async function loadAfresh(contents: Loadable, url: string): Promise<void> {
  await contents.loadURL(BLANK);
  await contents.loadURL(url);
  contents.navigationHistory.clear();
}
