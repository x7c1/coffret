// Where a window may go, and what becomes of a page's asking for another one.

import type { HandlerDetails, WindowOpenHandlerResponse } from 'electron';

/**
 * Whether a navigation to `target` stays on `origin`: the one origin a
 * window's page is served from. Anything that does not parse, or parses to an
 * opaque origin (`file:`, `javascript:`, `data:`), is refused.
 */
export function staysOn(origin: string | null, target: string): boolean {
  if (origin === null) {
    return false;
  }
  let url: URL;
  try {
    url = new URL(target);
  } catch {
    return false;
  }
  return (url.protocol === 'http:' || url.protocol === 'https:') && url.origin === origin;
}

/** What a window's contents offer for its navigations to be watched. */
export interface Navigable {
  on(
    event: 'will-navigate' | 'will-redirect',
    listener: (event: { preventDefault(): void }, url: string) => void,
  ): unknown;
}

/**
 * Refuses every navigation of `contents` that leaves the origin `origin()`
 * names at the time. Navigations the app makes itself (`loadURL`) are not
 * the page's, and are not asked about.
 *
 * What was refused is said by its kind alone: the URL a page tried could carry
 * anything, and the log keeps none of it.
 */
export function guardNavigation(
  contents: Navigable,
  window: string,
  origin: () => string | null,
  warn: (message: string) => void,
): void {
  const guard = (event: { preventDefault(): void }, url: string) => {
    if (!staysOn(origin(), url)) {
      event.preventDefault();
      warn(`the ${window} window was refused a navigation away from its origin`);
    }
  };
  contents.on('will-navigate', guard);
  contents.on('will-redirect', guard);
}

const DENY: WindowOpenHandlerResponse = { action: 'deny' };

/** The Passphrase window opens nothing. */
export function denyEveryWindow(): WindowOpenHandlerResponse {
  return DENY;
}

/**
 * What the explorer asking for a new window comes to: an `https:` address is
 * handed to the system browser — the Drive consent page, where Google asks for
 * the Library's access again — and every other is refused. No window of the
 * app's own is ever opened for it.
 */
export function explorerWindowOpener(
  openExternal: (url: string) => Promise<void>,
  warn: (message: string) => void,
): (details: Pick<HandlerDetails, 'url'>) => WindowOpenHandlerResponse {
  return ({ url }) => {
    if (isHttps(url)) {
      // Said without the error, which may quote the address: a consent page's
      // carries the state that ties Google's answer to this asking.
      openExternal(url).catch(() => {
        warn('the system browser could not be asked to open a page');
      });
    } else {
      warn('the explorer was refused a window for an address that is not https');
    }
    return DENY;
  };
}

function isHttps(target: string): boolean {
  try {
    return new URL(target).protocol === 'https:';
  } catch {
    return false;
  }
}

/** The explorer's address, with a launch ticket for its page to exchange in the fragment. */
export function explorerLaunchUrl(address: string, ticket: string): string {
  return `http://${address}/#launch=${ticket}`;
}

/** The origin the explorer at `address` (`127.0.0.1:<port>`) is served from. */
export function explorerOrigin(address: string): string {
  return new URL(`http://${address}/`).origin;
}
