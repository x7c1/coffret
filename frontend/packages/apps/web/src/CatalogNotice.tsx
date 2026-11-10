import { ReconnectOffer, type Offer } from './ReconnectOffer';
import { COLOR } from './theme';

/**
 * Why the rows below may not be the Library: the sentence
 * [`catalogLine`](./refresh) says, and, where the catch-up was refused because
 * the permission Storage is reached through ran out, the reconnect after it.
 *
 * Each sentence starts a line of its own — the catch-up's, then the
 * reconnect's under it — rather than running on into one long line.
 */
export function CatalogNotice({ said, offer }: { said: string; offer: Offer | null }) {
  return (
    <div style={NOTICE}>
      <div>{said}</div>
      {offer !== null && <ReconnectOffer offer={offer} />}
    </div>
  );
}

const NOTICE = {
  display: 'grid',
  gap: 6,
  padding: '8px 12px',
  borderBottom: `1px solid ${COLOR.border}`,
  background: COLOR.warnGround,
  color: COLOR.warn,
  fontSize: 13,
} as const;
