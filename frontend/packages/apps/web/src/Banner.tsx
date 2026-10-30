import type { ReactNode } from 'react';

import { dropLine, type DropOutcome } from './dropTarget';
import { COLOR, SHADOW } from './theme';

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
 * that folder's row outlined in the same colour — or, for rows of the tree
 * dragged onto a folder, `line` says where they move to instead (see
 * [`moveLine`](./moving)). It stands over the rows rather than among them,
 * and takes no pointer, so the drag goes on reading the row under it.
 */
export function DropPill({
  outcome,
  landing,
  line = dropLine(outcome, landing),
}: {
  outcome: DropOutcome;
  landing: string;
  line?: string;
}) {
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
          boxShadow: SHADOW.menu,
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          whiteSpace: 'nowrap',
        }}
      >
        {line}
      </span>
    </div>
  );
}

/**
 * The ring a drag over a drop target brings up, in the colour of its outcome:
 * drawn inside the edge (the outline is offset inward by its width), so that a
 * row's ring does not run under the rows either side of it.
 */
export function dropOutline(dragged: boolean, outcome: DropOutcome): string {
  if (!dragged) {
    return 'none';
  }
  return `2px solid ${RING[outcome]}`;
}

/** What the row a drop lands in is tinted with inside its ring, in the tone of its outcome. */
export function dropGround(outcome: DropOutcome): string {
  return GROUND[outcome];
}

/** The colour of a drop's outcome: taken, or refused. */
const TONE: Record<DropOutcome, string> = {
  taken: COLOR.accentDeep,
  taken_after: COLOR.accentDeep,
  refused: COLOR.refused,
};

/** The ring around a drop target: the accent itself where the drop is taken, its text shade being for words. */
const RING: Record<DropOutcome, string> = {
  taken: COLOR.accent,
  taken_after: COLOR.accent,
  refused: COLOR.refused,
};

/** What is under the drop pill, in the tone of its outcome. */
const GROUND: Record<DropOutcome, string> = {
  taken: COLOR.accentTint,
  taken_after: COLOR.accentTint,
  refused: COLOR.refusedGround,
};
