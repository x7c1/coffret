// How far the tree scrolls to keep the focused row away from its edges, kept
// free of DOM so it is unit testable.

/** How many rows the focused row is kept from either edge of the scrolling area. */
export const ROWS_AHEAD = 3;

/** Where the focused row and the scrolling area stand, in the area's own pixels. */
export interface ScrollGeometry {
  /** The row's top and bottom, measured from the top of what scrolls. */
  top: number;
  bottom: number;
  /** How far the area is scrolled now. */
  scrollTop: number;
  /** How tall the area is. */
  height: number;
  /**
   * How much of the area's top the block that stays put covers — the column
   * headings and the banners over the rows — which no row is read under.
   */
  covered: number;
  /** How far from either edge the row is kept: {@link ROWS_AHEAD} rows' height. */
  margin: number;
}

/**
 * Where to scroll so the focused row stands at least `margin` from the top
 * and the bottom of what is visible, or `null` where it does already.
 *
 * The way a desktop file manager scrolls while the arrow keys walk a long
 * list: the list moves ahead of the focus, so the rows it is about to reach
 * are on show before it reaches them, rather than the focus sliding to the
 * very edge and the list following one row behind.
 *
 * Moved the least distance that does it, so a row already well inside moves
 * nothing. Where the area is too short for a margin either side, the top one
 * wins: the row stays readable, and the rows above it are what `↑` reaches
 * next.
 */
export function scrollAhead(at: ScrollGeometry): number | null {
  const shownTop = at.scrollTop + at.covered;
  const shownBottom = at.scrollTop + at.height;
  if (at.top - at.margin < shownTop) {
    const to = Math.max(0, at.top - at.margin - at.covered);
    return to === at.scrollTop ? null : to;
  }
  if (at.bottom + at.margin > shownBottom) {
    return at.bottom + at.margin - at.height;
  }
  return null;
}
