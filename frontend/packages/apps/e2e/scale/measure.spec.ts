// The folder listing and the tree's `↓`, timed in the explorer over the large
// Library.
//
// Two of the three measurements `make large-measure` reports; the third, the
// full scan over the Index, is `coffret-measure`'s and was taken before the
// server started. Each appends its lines to the report the script prints.
//
// The listing is asked from the explorer's own tab, through the host, with the
// browser token the explorer itself sends: what is timed is what the explorer
// waits on, the server's answer and its reading included.
//
// The `↓` is timed from the key press to the frame that paints the new focus:
// the press's own event time, to a message posted from the first animation
// frame at which the row the focus moved to is drawn as focused — a message
// posted from a frame's callback is delivered once that frame has been
// rendered, which is the paint being waited for.

import { appendFileSync } from 'node:fs';

import { expect, test, type Page } from '@playwright/test';

import { openLaunchFile } from '../journeys/launch';
import { fromEnvironment } from './environment';
import { nextToOpen, reportLine, summarized } from './measured';

const setting = fromEnvironment();

/** How many times each listing is asked. */
const LISTING_RUNS = 7;

/** The tree's rows that stand for a folder or a file, not a note. */
const ROWS = '[role="tree"] tr[data-key^="folder:"], [role="tree"] tr[data-key^="file:"]';

/**
 * A closed folder's disclosure. Named by what it does, since the row's "⋮"
 * reports `aria-expanded` too, for its menu.
 */
const CLOSED = 'button[aria-label^="expand "][aria-expanded="false"]';

/** How long the page is given to settle between two measured presses. */
const SETTLE_TIMEOUT_MS = 30_000;

function report(line: string): void {
  console.log(line);
  appendFileSync(setting.report, `${line}\n`);
}

/** The part of a folder listing's answer that says what is below each child folder. */
interface Listed {
  folders: { path: string; files_total: number }[];
}

test('the listing and the tree over a hundred thousand files', async ({ page }) => {
  // The browser token, as the explorer sends it, and what each folder's
  // listing says is below each of its child folders.
  let authorization: string | undefined;
  const filesBelow = new Map<string, number>();
  page.on('request', (request) => {
    const sent = request.headers()['authorization'];
    if (sent !== undefined && new URL(request.url()).pathname.startsWith('/api/')) {
      authorization = sent;
    }
  });
  page.on('response', async (response) => {
    if (new URL(response.url()).pathname !== '/api/list' || !response.ok()) {
      return;
    }
    try {
      const listed = (await response.json()) as Listed;
      for (const folder of listed.folders) {
        filesBelow.set(folder.path, folder.files_total);
      }
    } catch {
      // A body the page let go of before it was read; the folder is opened
      // later than it might have been, and nothing is mismeasured.
    }
  });

  await openLaunchFile(page, setting.launchDir, { afterRestart: false });
  await expect(page.locator('footer')).toBeVisible();
  await expect(page.locator(ROWS).first()).toBeVisible();
  expect(authorization, 'the explorer asked the server something').toBeDefined();

  report('');
  report(
    `2. The folder listing the explorer uses (GET /api/list, read to the end and parsed in the tab, ${LISTING_RUNS} runs each)`,
  );
  for (const [what, folder] of [
    ['listing at the Library root', ''],
    [
      setting.largestFolder === '' ? 'listing at the largest folder (the root)' : 'listing at the largest folder',
      setting.largestFolder,
    ],
  ] as const) {
    const { millis, rows } = await timeListing(page, authorization ?? '', folder);
    report(reportLine(what, summarized(millis), `${rows} rows`));
  }

  report('');
  report(
    `3. The tree's ↓ with many rows open (key press to the frame that paints the new focus, ${setting.presses} presses each)`,
  );
  for (const target of setting.rows) {
    const opened = await openUntil(page, target, filesBelow);
    const millis = await timeArrowDown(page, setting.presses);
    report(reportLine(`↓ with ${opened} rows drawn`, summarized(millis)));
  }
});

/** Asks for `folder`'s listing `LISTING_RUNS` times from the tab. */
async function timeListing(
  page: Page,
  authorization: string,
  folder: string,
): Promise<{ millis: number[]; rows: number }> {
  return page.evaluate(
    async ({ authorization, folder, runs }) => {
      const url = folder === '' ? '/api/list' : `/api/list?${new URLSearchParams({ path: folder })}`;
      const millis: number[] = [];
      let rows = 0;
      for (let run = 0; run < runs; run += 1) {
        const started = performance.now();
        const response = await fetch(url, {
          headers: { Authorization: authorization },
          credentials: 'omit',
          cache: 'no-store',
        });
        if (!response.ok) {
          throw new Error(`the listing of "${folder}" was refused: ${response.status}`);
        }
        const listed = (await response.json()) as { folders: unknown[]; files: unknown[] };
        millis.push(performance.now() - started);
        rows = listed.folders.length + listed.files.length;
      }
      return { millis, rows };
    },
    { authorization, folder, runs: LISTING_RUNS },
  );
}

/**
 * Opens folders until at least `target` rows are drawn, the closed folder on
 * show with the most files below it first, and answers how many are.
 */
async function openUntil(page: Page, target: number, filesBelow: ReadonlyMap<string, number>): Promise<number> {
  for (;;) {
    const { rows, closed } = await page.evaluate(
      ({ selector, disclosure }) => {
        const closed = [...document.querySelectorAll(`[role="tree"] ${disclosure}`)]
          .map((button) => button.closest('tr')?.getAttribute('data-key') ?? '')
          .filter((key) => key.startsWith('folder:'))
          .map((key) => key.slice('folder:'.length));
        return { rows: document.querySelectorAll(selector).length, closed };
      },
      { selector: ROWS, disclosure: CLOSED },
    );
    if (rows >= target) {
      return rows;
    }
    const next = nextToOpen(closed, filesBelow);
    if (next === undefined) {
      throw new Error(`every folder is open and only ${rows} rows are drawn, short of ${target}`);
    }
    // Clicked in the page rather than through Playwright's own click, which
    // would scroll to it and wait on it as an action: what follows is the
    // listing arriving and being drawn, which is what is waited for.
    await page.evaluate(
      async ({ path, selector, disclosure, timeout }) => {
        const row = [...document.querySelectorAll('[role="tree"] tr[data-key]')].find(
          (candidate) => candidate.getAttribute('data-key') === `folder:${path}`,
        );
        const button = row?.querySelector(disclosure);
        if (!(button instanceof HTMLElement)) {
          throw new Error(`the folder "${path}" has no closed disclosure to open`);
        }
        const before = document.querySelectorAll(selector).length;
        button.click();
        const until = performance.now() + timeout;
        while (document.querySelectorAll(selector).length === before) {
          if (performance.now() > until) {
            throw new Error(`opening "${path}" drew no more rows within ${timeout}ms`);
          }
          await new Promise((resolve) => setTimeout(resolve, 20));
        }
      },
      { path: next, selector: ROWS, disclosure: CLOSED, timeout: SETTLE_TIMEOUT_MS },
    );
  }
}

/**
 * Focuses the first row, then times `presses` presses of `↓`, each from its
 * key event to the frame that painted the focus on the next row.
 */
async function timeArrowDown(page: Page, presses: number): Promise<number[]> {
  await page.locator(ROWS).first().click();
  await expect(page.locator('[role="tree"] tr[data-focused]')).toHaveCount(1);
  await settled(page);
  const millis: number[] = [];
  for (let press = 0; press < presses; press += 1) {
    // Armed before the key is sent, so the press's own event is the start.
    await page.evaluate(() => {
      const focusedKey = () =>
        document.querySelector('[role="tree"] tr[data-focused]')?.getAttribute('data-key') ?? null;
      const before = focusedKey();
      const measured = new Promise<number>((resolve) => {
        const onKey = (event: KeyboardEvent) => {
          if (event.key !== 'ArrowDown') {
            return;
          }
          window.removeEventListener('keydown', onKey, true);
          const pressedAt = event.timeStamp;
          const frame = () =>
            requestAnimationFrame(() => {
              if (focusedKey() === before) {
                frame();
                return;
              }
              const channel = new MessageChannel();
              channel.port1.onmessage = () => resolve(performance.now() - pressedAt);
              channel.port2.postMessage(null);
            });
          frame();
        };
        window.addEventListener('keydown', onKey, true);
      });
      (window as unknown as { coffretMeasured: Promise<number> }).coffretMeasured = measured;
    });
    await page.keyboard.press('ArrowDown');
    millis.push(
      await page.evaluate(() => (window as unknown as { coffretMeasured: Promise<number> }).coffretMeasured),
    );
    await settled(page);
  }
  return millis;
}

/**
 * Waits for the page to be idle: two animation frames in a row that come
 * within a frame and a half of each other, so that whatever the last press
 * set off — a listing for the folder the focus landed on, drawn over every
 * row — is not timed with the next one.
 */
async function settled(page: Page): Promise<void> {
  await page.evaluate(async (timeout) => {
    const until = performance.now() + timeout;
    let quiet = 0;
    let last = await new Promise<number>((resolve) => requestAnimationFrame(resolve));
    while (quiet < 2 && performance.now() < until) {
      const now = await new Promise<number>((resolve) => requestAnimationFrame(resolve));
      quiet = now - last < 25 ? quiet + 1 : 0;
      last = now;
    }
    // And a moment for a listing still on its way to arrive and be drawn.
    await new Promise((resolve) => setTimeout(resolve, 200));
    await new Promise((resolve) => requestAnimationFrame(resolve));
  }, SETTLE_TIMEOUT_MS);
}
