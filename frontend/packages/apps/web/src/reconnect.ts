// Renewing the grant Storage stopped taking, from wherever the refusal is.
//
// The server runs Google's consent flow itself (see `startReconnect`). What
// this page does is offer the button where the refusal is, open the consent
// page in a tab of its own, and follow the work answer for how it ended.
//
// Kept free of DOM and of React so it is unit testable, as `refresh` is.

import type { Reconnect, ReconnectState, Reconnecting } from '@coffret/api';

import { said } from './useAsked';

export { ranOut, refusedAsRanOut } from './ranOut';

/**
 * What the page says where Storage refused because the permission ran out.
 *
 * Said by the page rather than the server, because the server does not know
 * which provider the refusal came from and the page does: it is offered only
 * on a Library on Google Drive.
 */
export const RAN_OUT =
  "Google Drive's permission for this device ran out; it has to be renewed every seven days";

/** What is written on the button that renews it. */
export const RECONNECTING = 'reconnect';

/** Everything one press of the button reaches out to. */
export interface Pressing {
  /** Asks the server, which is [`startReconnect`](@coffret/api). */
  ask: () => Promise<Reconnecting>;
  /** Opens the consent page in a tab of its own. */
  open: (url: string) => void;
  /** Says what the press came to, and `null` clears what the last one said. */
  line: (line: string | null) => void;
  /** Says what refused it, and `null` clears what refused the last one. */
  trouble: (line: string | null) => void;
  /** Starts following the work answer, which is where the flow's ending lands. */
  follow: () => void;
}

/**
 * Asks the server to renew the permission, opens the page it answers with, and
 * follows the work answer, which is where the flow's ending lands.
 *
 * It never rejects. A refusal becomes the sentence beside the button, which is
 * what a person can act on.
 */
export async function askToReconnect(pressing: Pressing): Promise<void> {
  pressing.line(null);
  pressing.trouble(null);
  try {
    const answer = await pressing.ask();
    pressing.open(answer.url);
    pressing.line(answer.message);
    pressing.follow();
  } catch (refused: unknown) {
    pressing.trouble(said(refused));
  }
}

/** What the link to the consent page is labelled, for a tab that did not open. */
export const NO_TAB = 'no tab opened? open the consent page';

/**
 * The consent page to link to beside the line, or `null` where there is none.
 *
 * The page is opened once the server has answered, which is after the click
 * that asked for it, and a browser that blocks a tab opened outside a gesture
 * blocks that one without a word — `window.open` with `noopener` answers `null`
 * either way, so the page cannot tell. While the flow waits the button is gone,
 * so without this a blocked tab would leave nothing to do until the flow timed
 * out. A link is a gesture of its own, and the page it names is the one the
 * server is still listening behind for exactly as long as the flow waits.
 */
export function consentLink(reconnect: Reconnect | null, page: string | null): string | null {
  return reconnect?.state === 'waiting' ? page : null;
}

/** Whether the button is offered, which is whenever no flow is waiting. */
export function offersReconnect(reconnect: Reconnect | null): boolean {
  return reconnect?.state !== 'waiting';
}

/**
 * What the screen says beside the button about the last reconnect, or `null`
 * where nothing is to be said.
 *
 * `renewed` says nothing here: by then the refusal the button stood beside is
 * gone, and the line that says the permission was renewed is the notice's.
 */
export function reconnectLine(reconnect: Reconnect | null): string | null {
  if (reconnect === null) {
    return null;
  }
  switch (reconnect.state) {
    case 'waiting':
    case 'refused':
    case 'timed_out':
    case 'failed':
      return reconnect.message;
    case 'renewed':
      return null;
  }
}

/**
 * Whether the answer just heard is a reconnect that has landed: a flow that was
 * not renewed before and now is.
 *
 * Read the way [`catchUpLanded`](./refresh) is, against the last state this
 * window was told. Coming up to a server whose last reconnect was renewed —
 * nothing before this one — is not it: the page has just asked for the tree and
 * the folder.
 */
export function grantRenewed(before: ReconnectState | null, now: ReconnectState | null): boolean {
  return before !== null && before !== 'renewed' && now === 'renewed';
}
