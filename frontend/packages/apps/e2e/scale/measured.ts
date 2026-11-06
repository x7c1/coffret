// The arithmetic of the measurement, apart from the browser: what a set of
// timings comes to, how it is written in the report, and which folder the tree
// is opened at next.

/** The median and the worst of a set of timings, in milliseconds. */
export interface Summary {
  median: number;
  worst: number;
}

export function summarized(millis: readonly number[]): Summary {
  if (millis.length === 0) {
    throw new Error('nothing was timed');
  }
  const sorted = [...millis].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  const median = sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
  return { median, worst: sorted[sorted.length - 1] };
}

/** One line of the report, laid out as `coffret-measure` lays out its own. */
export function reportLine(what: string, summary: Summary, answered?: string): string {
  const ms = (value: number) => value.toFixed(1).padStart(9);
  const tail = answered === undefined ? '' : `  (${answered})`;
  return `   ${what.padEnd(40)} median ${ms(summary.median)} ms   worst ${ms(summary.worst)} ms${tail}`;
}

/**
 * Which of the closed folders on show to open next: the one with the most
 * files below it, as its parent's listing counted them, so that the tree
 * reaches a count of rows in as few openings as it can. Of two as large, the
 * one shown first. `undefined` where none is closed.
 *
 * A folder with many files below it is not always one that adds many rows —
 * its files may be a level further down — but it is on the way to them, and a
 * book's own count is exactly the rows it adds.
 */
export function nextToOpen(
  closed: readonly string[],
  filesBelow: ReadonlyMap<string, number>,
): string | undefined {
  let best: string | undefined;
  let most = -1;
  for (const path of closed) {
    const files = filesBelow.get(path) ?? 0;
    if (files > most) {
      best = path;
      most = files;
    }
  }
  return best;
}
