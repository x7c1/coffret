// What `scripts/large.sh measure` hands over.
//
// Nothing here has a default, for the reason the journeys' environment has
// none: the measurement is one half of `make large-measure` — the other half
// seeded the Library and started the server and the host over it — so a
// variable that is missing means it was started on its own.

/** What `scripts/large.sh measure` hands over. */
export interface Environment {
  /** Where the explorer is, as a page addresses it. */
  webUrl: string;
  /** The owner-only directory the host writes its launch file into. */
  launchDir: string;
  /** The folder with the most rows, as an Entry Path; empty for the root. */
  largestFolder: string;
  /** The rows the tree is opened to before `↓` is timed, in ascending order. */
  rows: number[];
  /** How many presses of `↓` each count of rows is timed over. */
  presses: number;
  /** The plain report the numbers are appended to. */
  report: string;
  /** Where Playwright's own traces and failure shots go. */
  artifacts: string;
  /** A Chromium to run instead of Playwright's own, where one is named. */
  chromium: string | undefined;
}

/** What the script said, or a refusal naming what it did not say. */
export function fromEnvironment(): Environment {
  const rows = required('COFFRET_LARGE_ROWS')
    .split(',')
    .map((count) => Number(count.trim()));
  if (rows.some((count) => !Number.isInteger(count) || count <= 0)) {
    throw new Error(`COFFRET_LARGE_ROWS is a list of row counts, such as 5000,20000`);
  }
  const presses = Number(required('COFFRET_LARGE_PRESSES'));
  if (!Number.isInteger(presses) || presses <= 0) {
    throw new Error('COFFRET_LARGE_PRESSES is a count of presses above zero');
  }
  return {
    webUrl: required('COFFRET_LARGE_WEB_URL'),
    launchDir: required('COFFRET_LARGE_LAUNCH_DIR'),
    // Empty where the root is the largest, which is a value and not a miss.
    largestFolder: process.env.COFFRET_LARGE_FOLDER ?? '',
    rows: [...rows].sort((a, b) => a - b),
    presses,
    report: required('COFFRET_LARGE_REPORT'),
    artifacts: required('COFFRET_LARGE_ARTIFACTS'),
    chromium: process.env.COFFRET_CHROMIUM || undefined,
  };
}

function required(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === '') {
    throw new Error(
      `${name} is not set. The measurement is run by \`make large-measure\` ` +
        '(scripts/large.sh), which sets it.',
    );
  }
  return value;
}
