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
 * Said while files are dragged over the tree: a pill at the bottom of it,
 * naming the folder letting go of them there adds to (see `dropLine`), with
 * that folder's row outlined in the same colour. It stands over the rows
 * rather than among them, and takes no pointer, so the drag goes on reading
 * the row under it.
 */
export function DropPill({ outcome, landing }: { outcome: DropOutcome; landing: string }) {
  return (
    <div
      role="status"
      style={{
        position: 'absolute',
        left: 0,
        right: 0,
        bottom: 12,
        display: 'flex',
        justifyContent: 'center',
        pointerEvents: 'none',
        zIndex: 2,
      }}
    >
      <span
        style={{
          maxWidth: 'calc(100% - 32px)',
          padding: '5px 14px',
          borderRadius: 999,
          border: `1px solid ${TONE[outcome]}`,
          background: GROUND[outcome],
          color: TONE[outcome],
          fontSize: 13,
          boxShadow: '0 4px 16px rgba(0, 0, 0, 0.5)',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}
      >
        {dropLine(outcome, landing)}
      </span>
    </div>
  );
}

/** The outline a drag over a drop target brings up, in the colour of its outcome. */
export function dropOutline(dragged: boolean, outcome: DropOutcome): string {
  if (!dragged) {
    return 'none';
  }
  return `2px dashed ${TONE[outcome]}`;
}

/** The colour of a drop's outcome: taken, or refused. */
const TONE: Record<DropOutcome, string> = {
  taken: COLOR.added,
  taken_after: COLOR.added,
  refused: COLOR.refused,
};

/** What is under the drop pill, in the tone of its outcome. */
const GROUND: Record<DropOutcome, string> = {
  taken: COLOR.addedGround,
  taken_after: COLOR.addedGround,
  refused: COLOR.refusedGround,
};
