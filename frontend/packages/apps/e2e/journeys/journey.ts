// What every journey is written against: the server, the pictures, and the few
// places on the screen the journeys reach for.
//
// The helpers here name parts of the explorer and nothing about the wire. A
// journey asserts what a person would see — a chip that says `remote`, a page
// in the reader, a line in the status bar — and never what a route answered
// with: the routes are stage one's to check, in `scripts/e2e-it.sh`, and a
// second reading of them here would be a second thing to keep in step with the
// server.

import path from 'node:path';

import { test as base, type Locator, type Page } from '@playwright/test';

import { fromEnvironment, type Environment } from './environment';
import { CoffretServer } from './server';

/** What the run built, read once and shared by every journey. */
export const setting: Environment = fromEnvironment();

/**
 * The one server, started before the first journey and killed after the last.
 *
 * Worker-scoped and automatic, because the journeys are one journey: they share
 * a Library on disk and run in one worker, in order. The fixture is handed over
 * so that the outage journey can kill it and start it again — which is the only
 * reason the process is the suite's rather than the script's.
 */
export const test = base.extend<object, { coffret: CoffretServer }>({
  coffret: [
    // Playwright reads which other fixtures a fixture depends on out of this
    // pattern, so it has to be one even where nothing is taken from it.
    // eslint-disable-next-line no-empty-pattern
    async ({}, use) => {
      const server = new CoffretServer(setting);
      await server.start();
      await use(server);
      await server.stop();
    },
    { scope: 'worker', auto: true },
  ],
});

export { expect } from '@playwright/test';

/**
 * A picture of where the journey has got to, for a person to look at.
 *
 * Nothing compares these and nothing asserts on them beyond their existing:
 * whether the filer reads right, whether the chips are legible, whether the
 * outage notice says something a person could act on, are all questions only
 * eyes answer. What the journeys assert is what the page says; these are what
 * is left over, and they are the point of the target having a browser stage at
 * all.
 *
 * The folder is named after the spec file it was taken from, with the number
 * that orders the files taken off: the file is the journey, so nothing has to
 * repeat its name at each checkpoint and nothing can drift from it.
 */
export async function shot(page: Page, checkpoint: string): Promise<void> {
  const file = path.basename(test.info().file).replace(/\.spec\.ts$/, '');
  const journey = file.replace(/^\d+-/, '');
  await page.screenshot({ path: path.join(setting.screenshots, journey, `${checkpoint}.png`) });
}

/** The folder tree, down the left. */
export function tree(page: Page): Locator {
  return page.getByRole('navigation', { name: 'folders' });
}

/**
 * One folder in the tree, by name.
 *
 * The name and not the marker beside it: the marker opens and closes the
 * branch, and what a journey walking to a folder means is the folder.
 */
export function inTree(page: Page, name: string): Locator {
  return tree(page).getByTitle(name, { exact: true });
}

/**
 * One row of the current folder's listing, by the name it shows.
 *
 * By the row's own `title`, which the explorer puts the file's name in so that
 * a name too long for the column can still be read. It is the whole name and
 * not part of one, which is what makes `img-00001.jpg` a different row from
 * `img-00011.jpg`.
 */
export function row(page: Page, name: string): Locator {
  return page.locator(`tbody tr[title="${name}"]`);
}

/**
 * What one row says its state is.
 *
 * The chip, which is the row's one word about itself: `present`, `remote`,
 * `uploading` while a sync has yet to carry it in, and `fetching`, `failed` or
 * `declined` while the server is bringing the folder over.
 */
export function chip(page: Page, name: string): Locator {
  return row(page, name).locator('span');
}

/** The last component of an Entry Path, which is what a folder is called. */
export function leaf(entry: string): string {
  return entry.slice(entry.lastIndexOf('/') + 1);
}

/** Its first, which is the part of the Library a mapping is keyed by. */
export function top(entry: string): string {
  const cut = entry.indexOf('/');
  return cut === -1 ? entry : entry.slice(0, cut);
}

/** The name of the photo at one index of the generated album. */
export function photo(index: number): string {
  return `img-${String(index).padStart(5, '0')}.jpg`;
}

/** The name of the page at one index of the generated book. */
export function bookPage(index: number): string {
  return `page-${String(index).padStart(4, '0')}.jpg`;
}

/**
 * Waits a moment for something to be on the screen, and carries on either way.
 *
 * For the lines that say what is happening *while* it is happening, which the
 * picture taken after this is meant to catch. They are not asserted, and that
 * is deliberate: how long a fill or a sync takes is the machine's business, and
 * a journey that failed because the work was quick would be reporting a fast
 * disk as a broken explorer. What the journeys assert is the outcome, which is
 * the same everywhere.
 */
export async function glimpse(locator: Locator, ms: number): Promise<void> {
  try {
    await locator.first().waitFor({ state: 'visible', timeout: ms });
  } catch {
    // It was over before this looked, or it never started. Neither is something
    // to report: what the picture then shows is whatever the screen shows, and
    // the assertion around it has not moved.
  }
}

/**
 * Drops one file from this disk onto part of the screen.
 *
 * Through the browser's own drag machinery rather than through an event this
 * suite builds, and that is not a detail: the explorer reads a drop with
 * `webkitGetAsEntry`, because that is the only way a dropped *folder* can be
 * walked — and a `DataTransfer` a page constructs carries no entry at all, so a
 * fabricated drop would arrive as a gesture that carried nothing. What this
 * sends is a real drag of a real file, which is what a person does.
 *
 * One file, and no folder. A folder would have to be dragged in as a folder,
 * which this cannot express; the walk that reads one is unit-tested, and a
 * folder drop stays something a person checks by hand.
 */
export async function dropFileOnto(page: Page, target: Locator, file: string): Promise<void> {
  const box = await target.boundingBox();
  if (box === null) {
    throw new Error('nothing to drop onto: the target is not on the screen');
  }
  const at = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  // `dragOperationsMask: 1` is "copy", which is what dropping a file on
  // something that is not a file manager means.
  const data = { items: [], files: [file], dragOperationsMask: 1 };
  const session = await page.context().newCDPSession(page);
  try {
    for (const type of ['dragEnter', 'dragOver', 'drop'] as const) {
      await session.send('Input.dispatchDragEvent', { type, ...at, data });
    }
  } finally {
    await session.detach();
  }
}
