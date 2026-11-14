// The Activity panel holds still while what it says changes, and the reader's
// slide draws no horizontal scrollbar under it.
//
// While something runs, the panel's In progress line — and the collapsed
// row's summary — turns from a run's line into the reader's fetch and back as
// a book is read. What it says changes; nothing else about the panel may: not
// its height, and not where the parts beside the words stand. And the reader's
// sheet slides in from the right as it opens: part-way in, it must not widen
// the page, which would draw a horizontal scrollbar for the length of the
// slide.
//
// Both run and fetch are held still on purpose, so that each state is there to
// be measured for as long as it takes: the reader's request for one file is
// held in the browser until the journey lets it go, and the run is a fill the
// journey writes into the server's answer about its work — one that has begun
// committing, whose line has no Stop, beside a fetch whose line has one. The
// server's own answer goes through untouched once the journey stops writing
// it in.

import type { Locator, Page, Route } from '@playwright/test';

import { expect, openActivity, photo, row, setting, shot, test } from './journey';

/** The folder the written-in fill says it is bringing over. */
const FILLED = 'activity-steady';

/** What the written-in fill's line says, collapsed or not. */
const FILL_SAYS = new RegExp(`^bringing over 3/10 in ${FILLED}…`);

/**
 * A fill part-way through and committing, as `GET /api/work` would describe
 * one: its line has a bar and, in its Stop's place, `Finishing…`.
 */
const FILL = {
  run: 9001,
  folder: FILLED,
  total: 10,
  done: 3,
  declined: [],
  findings: [],
  stop: 'committing',
  waiting: [],
  discarded: [],
  displaced: [],
  status: 'filling',
  stopped: null,
} as const;

/**
 * Writes the fill above into every answer about the server's work while
 * `on()` says so, and lets the server's own answer through otherwise.
 */
async function writeInFill(page: Page, on: () => boolean): Promise<void> {
  await page.route(
    (url) => url.pathname === '/api/work',
    async (route) => {
      if (!on()) {
        await route.continue();
        return;
      }
      const answer = await route.fetch();
      const work = (await answer.json()) as Record<string, unknown>;
      await route.fulfill({ response: answer, json: { ...work, fill: FILL } });
    },
  );
}

/**
 * Holds the reader's requests for the file at `path` until the returned
 * function is called, and lets every other file through.
 */
async function holdFile(page: Page, path: string): Promise<() => void> {
  let release: () => void = () => undefined;
  const released = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(
    (url) => url.pathname === '/api/file' && url.searchParams.get('path') === path,
    async (route: Route) => {
      await released;
      await route.continue();
    },
  );
  return release;
}

/** The Activity panel along the bottom of the screen. */
function panel(page: Page): Locator {
  return page.locator('footer[aria-label="Activity"]');
}

/** Where `locator` is drawn, where it is drawn at all. */
async function boxOf(locator: Locator): Promise<{ x: number; y: number; width: number; height: number }> {
  const box = await locator.boundingBox();
  expect(box).not.toBeNull();
  return box ?? { x: 0, y: 0, width: 0, height: 0 };
}

/**
 * The collapsed row, measured: the panel's height, and where the parts after
 * the summary stand — the first of them (the bar's place; on a row that keeps
 * no place for the bar, whatever comes next), the failures' badge where there
 * is one, and the sync icon at the end.
 */
async function collapsedRow(
  page: Page,
): Promise<{ height: number; next: number; badge: number | null; sync: number }> {
  const footer = panel(page);
  const badge = footer.getByRole('button', { name: /^Show the failures/ });
  const sync = footer.locator('button').last();
  // The row's second child: its first is the toggle and the summary, together.
  const next = footer.locator(':scope > div:first-child > :nth-child(2)');
  return {
    height: (await boxOf(footer)).height,
    next: (await boxOf(next)).x,
    badge: (await badge.count()) === 0 ? null : (await boxOf(badge)).x,
    sync: (await boxOf(sync)).x,
  };
}

test('the Activity panel holds still as a run turns into a fetch and back', async ({ page }) => {
  let filling = true;
  await writeInFill(page, () => filling);
  const shown = `${setting.album}/${photo(0)}`;
  const release = await holdFile(page, shown);

  await page.goto(`/#path=${setting.album}`);
  await page.reload();
  await expect(row(page, photo(0))).toBeVisible();

  // Collapsed, saying the run, with its bar.
  const footer = panel(page);
  await expect(footer.getByText(FILL_SAYS)).toBeVisible();
  await expect(footer.getByRole('progressbar')).toBeVisible();
  const run = await collapsedRow(page);
  await shot(page, '01-collapsed-saying-a-run');

  // The run ends as the reader opens a file and fetches it: the row says the
  // fetch, which has no bar, and nothing beside the words moves.
  filling = false;
  await row(page, photo(0)).dblclick();
  const fetching = footer.getByText(`fetching ${photo(0)}…`);
  await expect(fetching).toBeVisible();
  await expect(footer.getByText(FILL_SAYS)).toHaveCount(0);
  const fetch = await collapsedRow(page);
  expect(fetch).toEqual(run);
  await shot(page, '02-collapsed-saying-a-fetch');

  // Open, the In progress line is the fetch's, with its Stop.
  await openActivity(page);
  const inProgress = footer.getByRole('region', { name: 'In progress' });
  const stop = inProgress.getByRole('button', { name: `Stop — fetching ${photo(0)}…` });
  await expect(stop).toBeVisible();
  const fetchHeight = (await boxOf(footer)).height;
  const fetchLines = (await boxOf(inProgress)).height;
  const stopBox = await boxOf(stop);
  await shot(page, '03-open-saying-a-fetch');

  // The fetch lands as the run begins committing again: the line turns into
  // the run's, with its bar and `Finishing…` where the Stop was — and the
  // panel keeps its height, the words in the Stop's place ending where the
  // Stop did.
  filling = true;
  release();
  const finishing = inProgress.getByText('Finishing…');
  await expect(finishing).toBeVisible();
  await expect(inProgress.getByText(FILL_SAYS)).toBeVisible();
  await expect(inProgress.getByText(/^fetching /)).toHaveCount(0);
  expect((await boxOf(inProgress)).height).toBe(fetchLines);
  expect((await boxOf(footer)).height).toBe(fetchHeight);
  const finishingBox = await boxOf(finishing);
  expect(finishingBox.x + finishingBox.width).toBeCloseTo(stopBox.x + stopBox.width, 0);
  await shot(page, '04-open-saying-a-run');

  // And once nothing runs, the line that says so is as tall as the one before.
  filling = false;
  await expect(inProgress.getByText('Nothing is running.')).toBeVisible({ timeout: 60_000 });
  expect((await boxOf(inProgress)).height).toBe(fetchLines);
  expect((await boxOf(footer)).height).toBe(fetchHeight);
  await shot(page, '05-open-saying-nothing-runs');
});

test('the reader slides in with the Activity panel open and the page never scrolls across', async ({
  page,
}) => {
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, photo(3))).toBeVisible();
  await openActivity(page);
  // Focused first, and opened by `Enter` once the list has scrolled ahead of
  // the focus: a double-click near the bottom of the shortened tree would
  // land its second click on whichever row that scroll brought under it.
  await row(page, photo(3)).click();
  await expect(row(page, photo(3))).toHaveAttribute('aria-selected', 'true');

  // Every frame from just before the press until well after the slide's
  // 160ms: how far the page was wider than the window, and how many frames
  // caught the sheet part-way in, so that a slide nobody sampled cannot pass.
  await page.evaluate(() => {
    const sampled = { over: 0, sliding: 0, done: false };
    (window as unknown as { sampled: typeof sampled }).sampled = sampled;
    const until = performance.now() + 1500;
    const frame = () => {
      const root = document.documentElement;
      sampled.over = Math.max(sampled.over, root.scrollWidth - root.clientWidth);
      const sheet = document.querySelector('section.sheet');
      if (sheet !== null && getComputedStyle(sheet).transform !== 'none') {
        sampled.sliding += 1;
      }
      if (performance.now() < until) {
        requestAnimationFrame(frame);
      } else {
        sampled.done = true;
      }
    };
    requestAnimationFrame(frame);
  });
  await page.keyboard.press('Enter');
  const reader = page.getByRole('region', { name: 'Reader' });
  await expect(reader.getByRole('img', { name: photo(3) })).toBeVisible();
  await page.waitForFunction(() => (window as unknown as { sampled: { done: boolean } }).sampled.done);
  const sampled = await page.evaluate(
    () => (window as unknown as { sampled: { over: number; sliding: number } }).sampled,
  );
  expect(sampled.sliding, 'the slide was sampled part-way in').toBeGreaterThan(0);
  expect(sampled.over, 'the page never grew wider than the window').toBe(0);
  await shot(page, '06-the-reader-open-over-the-open-panel');
});
