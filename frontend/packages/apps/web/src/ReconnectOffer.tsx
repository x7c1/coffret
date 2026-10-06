import type { Reconnect } from '@coffret/api';

import {
  consentLink,
  NO_TAB,
  offersReconnect,
  RAN_OUT,
  reconnectLine,
  RECONNECTING,
} from './reconnect';
import { COLOR } from './theme';

/** The one reconnect a screen offers, wherever the refusal it answers stands. */
export interface Offer {
  /** How the last reconnect stands, as the work answer says. */
  reconnect: Reconnect | null;
  /** What the last press came to, and `null` before any. */
  said: string | null;
  /** What refused the last press, and `null` where none was. */
  refused: string | null;
  /** The consent page the last press was answered with, and `null` once its flow stops waiting. */
  consentPage: string | null;
  /** Presses the button. */
  ask: () => void;
}

/**
 * What a refusal saying the permission ran out is drawn with: the sentence
 * saying so, the one button that renews it, and what the last press came to.
 *
 * One component for the four places a Storage refusal stands — the catch-up
 * notice, a region that could not be read, the reader, and the status bar —
 * because they are one offer: a press in any of them starts the one flow the
 * server runs, and a second press while it waits is answered with the same
 * consent page. So every one of them shows the same state of it.
 *
 * The button is not offered while a flow waits. The consent page is already
 * open, and what to do is answer it; the line beside where the button was says
 * so, and a link to the same page stands after it for a browser that did not
 * open the tab (see [`consentLink`](./reconnect)).
 */
export function ReconnectOffer({ offer }: { offer: Offer }) {
  const line = offer.refused ?? reconnectLine(offer.reconnect) ?? offer.said;
  const link = consentLink(offer.reconnect, offer.consentPage);
  return (
    <span style={{ display: 'inline-flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
      <span>{RAN_OUT}</span>
      {offersReconnect(offer.reconnect) && (
        <button onClick={offer.ask} style={BUTTON}>
          {RECONNECTING}
        </button>
      )}
      {line !== null && (
        <span style={{ color: offer.refused !== null ? COLOR.refused : COLOR.text }}>{line}</span>
      )}
      {link !== null && (
        <a href={link} target="_blank" rel="noopener noreferrer" style={{ color: COLOR.text }}>
          {NO_TAB}
        </a>
      )}
    </span>
  );
}

const BUTTON = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '1px 10px',
  borderRadius: 4,
  cursor: 'pointer',
} as const;
