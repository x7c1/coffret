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
 * Said while files are dragged over the tree — over a folder's row, or over the
 * rest of it for the folder the focus is in: what letting go of them there will do, as `dropLine` words
 * it. A folder that takes no drop says so in the refused colour the outline
 * around it is drawn in, and one not known yet either way in a neutral one.
 */
export function DropBanner({ outcome, held }: { outcome: DropOutcome; held: boolean }) {
  return (
    <Banner tone={TONE[outcome]} background={GROUND[outcome]}>
      {dropLine(outcome, held)}
    </Banner>
  );
}

/** The outline a drag over a drop target brings up, in the colour of its outcome. */
export function dropOutline(dragged: boolean, outcome: DropOutcome): string {
  if (!dragged) {
    return 'none';
  }
  return `2px dashed ${TONE[outcome]}`;
}

/**
 * The colour of a drop's outcome: refused, taken, or neither while it is not
 * known yet — so that no row shows one colour the instant a drag starts and
 * flips to the other when the answer arrives.
 */
const TONE: Record<DropOutcome, string> = {
  taken: COLOR.added,
  taken_after: COLOR.added,
  refused: COLOR.refused,
  unknown: COLOR.dim,
};

/** What is under the drop line, in the tone of its outcome. */
const GROUND: Record<DropOutcome, string> = {
  taken: COLOR.addedGround,
  taken_after: COLOR.addedGround,
  refused: COLOR.refusedGround,
  unknown: COLOR.panel,
};
