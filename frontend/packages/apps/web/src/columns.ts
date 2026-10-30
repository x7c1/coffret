// How wide the tree's value columns are drawn, kept free of DOM so it is unit
// testable.

/** Which of a column's strings is measured: its heading, or a value under it. */
export type Measured = 'heading' | 'value';

/**
 * How wide one value column is drawn: as wide as the widest of its heading and
 * the values among the rows on show, plus the cell's padding.
 *
 * Measured from the strings as they are rendered — each in its own font, the
 * heading's and the rows' — rather than estimated from a count of characters,
 * so a left-aligned heading and the values under it span the same width, and
 * a size like `999.9 MB` is not clipped where `1.0 kB` was guessed at.
 */
export function columnWidth(
  heading: string,
  values: readonly string[],
  measure: (text: string, measured: Measured) => number,
  padding: number,
): number {
  let widest = measure(heading, 'heading');
  for (const value of values) {
    widest = Math.max(widest, measure(value, 'value'));
  }
  return Math.ceil(widest) + padding;
}

/**
 * A measure of rendered text, in the fonts given, through a canvas the page
 * never shows.
 *
 * `null` where the browser offers no canvas to measure with — a test's DOM —
 * and the caller falls back to a measure of its own.
 */
export function canvasMeasure(fonts: Record<Measured, string>): ((text: string, measured: Measured) => number) | null {
  const context = drawingContext();
  if (context === null) {
    return null;
  }
  return (text, measured) => {
    context.font = fonts[measured];
    return context.measureText(text).width;
  };
}

/** The one canvas context every measure shares, made the first time one is asked for. */
let context: CanvasRenderingContext2D | null | undefined;

function drawingContext(): CanvasRenderingContext2D | null {
  if (context === undefined) {
    // A DOM without a canvas implementation says so rather than drawing:
    // jsdom answers `getContext` with a complaint on the console and `null`.
    const canvas = typeof document === 'undefined' ? null : document.createElement('canvas');
    context = canvas !== null && !/jsdom/i.test(navigator.userAgent) ? canvas.getContext('2d') : null;
  }
  return context;
}
