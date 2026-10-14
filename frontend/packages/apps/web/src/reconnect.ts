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
 * What stands where the link was, once the flow waiting is not the one this
 * window's press was answered with: a press in another window started it, and
 * its page went to that window alone.
 */
export const OPENED_ELSEWHERE =
  "Google's consent page now waiting was opened from another window; answer it there";

/**
 * The name the work answer gives a flow waiting on `page`: the 32-bit FNV-1a
 * digest of the page's UTF-8 bytes, as eight lowercase hex digits.
 *
 * The server computes the same digest over the page it waits on, and the work
 * answer carries that rather than the page, which goes only to the window that
 * pressed. Held to the server's by the waiting sample in the gateway's
 * `contract/work.json`, whose page is `https://consent.example/`.
 */
export function flowOf(page: string): string {
  let digest = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(page)) {
    digest = Math.imul(digest ^ byte, 0x01000193);
  }
  return (digest >>> 0).toString(16).padStart(8, '0');
}

/**
 * Whether the flow waiting is the one on `page`, the consent page this window's
 * own press was answered with.
 *
 * The state alone cannot say: one flow can end and another begin between two
 * polls — a press in another window — which reads `waiting` both times, and the
 * page this window holds then leads to a listener that has stopped.
 */
function waitsOn(reconnect: Reconnect | null, page: string | null): page is string {
  return reconnect?.state === 'waiting' && page !== null && reconnect.flow === flowOf(page);
}

/**
 * The consent page to link to beside the line, or `null` where there is none.
 *
 * Only the page this window's own press was answered with, and only while the
 * flow waiting is the one on it (see `waitsOn`): a flow another window started
 * waits behind a page this one was never handed, and the old page would send
 * an answer to nobody.
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
  return waitsOn(reconnect, page) ? page : null;
}

/**
 * Whether the link this window drew has been dropped because the flow waiting
 * is another window's, which is when [`OPENED_ELSEWHERE`] stands in its place.
 */
export function openedElsewhere(reconnect: Reconnect | null, page: string | null): boolean {
  return reconnect?.state === 'waiting' && page !== null && !waitsOn(reconnect, page);
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
