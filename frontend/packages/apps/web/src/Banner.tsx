import type { ReactNode } from 'react';

import { dropLine, type DropOutcome } from './dropTarget';
import { COLOR } from './theme';

/**
 * What a banner over the rows is drawn as.
 *
 * Not sticky of its own: the block the list draws its banners in is, so that
 * two standing at once stack rather than overlap as the rows scroll under them.
 */
export function Banner({
  tone,
  background,
  children,
}: {
  tone: string;
  background: string;
  children: ReactNode;
}) {
  return (
    <p
      style={{
        margin: 0,
        padding: '8px 12px',
        background,
        borderBottom: `1px solid ${tone}`,
        color: tone,
        fontSize: 13,
      }}
    >
      {children}
    </p>
  );
}

/**
 * Said while files are dragged over a drop target — the file list, or a row of
 * the folder tree: what letting go of them there will do, as `dropLine` words
 * it. A folder that takes no drop says so in the refused colour the outline
 * around it is drawn in.
 */
export function DropBanner({ outcome, held }: { outcome: DropOutcome; held: boolean }) {
  const refused = outcome === 'refused';
  return (
    <Banner
      tone={refused ? COLOR.refused : COLOR.added}
      background={refused ? COLOR.refusedGround : COLOR.addedGround}
    >
      {dropLine(outcome, held)}
    </Banner>
  );
}

/** The outline a drag over a drop target brings up, in the colour of its outcome. */
export function dropOutline(dragged: boolean, outcome: DropOutcome): string {
  if (!dragged) {
    return 'none';
  }
  return `2px dashed ${outcome === 'refused' ? COLOR.refused : COLOR.added}`;
}
