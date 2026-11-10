// What every journey is written against: the server, the pictures, and the few
// places on the screen the journeys reach for.
//
// The helpers here name parts of the explorer and nothing about the wire. A
// journey asserts what a person would see — a row on this device, a page
// in the reader, a line in the Activity panel — and never what a route answered
// with: the routes are stage one's to check, in `scripts/e2e-it.sh`, and a
// second reading of them here would be a second thing to keep in step with the
// server.

import path from 'node:path';

import { expect, test as base, type Locator, type Page } from '@playwright/test';

import { fromEnvironment, type Environment } from './environment';
import { openLaunchFile, serverStarting } from './launch';
import { CoffretServer } from './server';

/** What the run built, read once and shared by every journey. */
export const setting: Environment = fromEnvironment();

/** What a tab with no session shows in place of the Library. */
export const NO_SESSION = /The explorer holds no session/;

/**
 * Opens the explorer in `page`, the way a person does: by opening the launch file the
 * explorer's host wrote, which refreshes to the explorer with a one-time ticket
 * in the fragment. The page exchanges it and takes it out of the address.
 *
 * `afterRestart` waits for the file the host writes for a server of the next
 * run, rather than taking one written for the run before, whose generation the
 * host has since retired.
 */
export async function openExplorer(page: Page, { afterRestart = false } = {}): Promise<void> {
  await openLaunchFile(page, setting.launchDir, { afterRestart });
  // The Activity panel is drawn only for a tab holding a session.
  await expect(page.locator('footer')).toBeVisible();
  // The ticket is gone from the address the tab is left at.
  await expect(page).toHaveURL(setting.webUrl);
}

/**
 * The one server, started before the first journey and killed after the last,
 * and a page holding a session with the explorer in front of it for each journey.
 *
 * The server is worker-scoped and automatic, because the journeys are one
 * journey: they share a Library on disk and run in one worker, in order. The
 * fixture is handed over so that the journeys that take it away can kill it
 * and start it again — which is the only reason the process is the suite's
 * rather than the script's.
 *
 * The page opens the explorer before the journey begins. Nothing ends its
 * session after: the page offers no control that would, and a session a closed
 * page leaves behind is the one the host lets go of first once it holds as many
 * as it keeps.
 */
export const test = base.extend<object, { coffret: CoffretServer }>({
  coffret: [
    // Playwright reads which other fixtures a fixture depends on out of this
    // pattern, so it has to be one even where nothing is taken from it.
    // eslint-disable-next-line no-empty-pattern
    async ({}, use) => {
      const server = new CoffretServer(setting);
      // A worker started after a journey failed finds the launch file of the
      // last worker's server still standing, its ticket spent with that server;
      // this notes it, so that no journey opens it.
      await serverStarting(setting.launchDir);
      await server.start();
      await use(server);
      await server.stop();
    },
    { scope: 'worker', auto: true },
  ],
  page: async ({ page }, use) => {
    // Each journey starts unfiltered whatever the one before it left kept, so
    // that none depends on the last having cleared its filter.
    const read = keptFilterRead(page);
    await openExplorer(page);
    const kept = await read;
    if (kept !== null && (kept.name !== '' || kept.recentOn)) {
      await clearFilter(page);
    }
    await use(page);
  },
});

export { expect } from '@playwright/test';

/**
 * A picture of where the journey has got to, for a person to look at.
 *
 * Nothing compares these and nothing asserts on them beyond their existing:
 * whether the filer reads right, whether the state icons are legible, whether the
 * outage notice says something a person could act on, are all questions only
 * eyes answer. What the journeys assert is what the page says; these are what
 * is left over, and they are the point of the target having a browser stage at
 * all.
 *
 * The folder is named after the spec file it was taken from, number and all:
 * the file is the journey, so nothing has to repeat its name at each checkpoint
 * and nothing can drift from it — and the number makes a plain `ls` list the
 * journeys in the order they ran, which is the order a person reviews them in.
 */
export async function shot(page: Page, checkpoint: string): Promise<void> {
  const journey = path.basename(test.info().file).replace(/\.spec\.ts$/, '');
  await page.screenshot({ path: path.join(setting.screenshots, journey, `${checkpoint}.png`) });
}

/**
 * Opens the Activity panel along the bottom, where it is not open already:
 * what the server finished, and the Retry on a failure, are listed there.
 */
export async function openActivity(page: Page): Promise<void> {
  const toggle = page.getByRole('button', { name: 'Activity', exact: true });
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') {
    await toggle.click();
  }
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
}

/**
 * The Library's name, where the header says it at the top left: the name
 * alone, without the Storage it is kept on.
 */
export function libraryName(page: Page): Locator {
  return page.getByRole('banner').getByText(setting.library, { exact: true });
}

/** The tree, in the middle of the screen: the Library's folders and files as rows of one table. */
export function tree(page: Page): Locator {
  return page.getByRole('tree', { name: 'Library' });
}

/**
 * One folder's row in the tree, by name.
 *
 * By the row's own `title`, as [`row`](#row) finds a file, and narrowed to the
 * rows that draw a folder: the Library admits a file and a folder of one name,
 * and what a journey walking to a folder means is the folder.
 */
export function inTree(page: Page, name: string): Locator {
  return tree(page)
    .locator(`tr[title="${name}"]`)
    .filter({ has: page.getByRole('img', { name: 'folder', exact: true }) });
}

/**
 * Walks into one folder the way a person does: a click on its row, which puts
 * the focus there, and its disclosure button pressed where it is closed, which
 * draws its rows under it. A folder open and focused is what the screen's own
 * actions, and the URL, are about.
 */
export async function openInTree(page: Page, name: string): Promise<void> {
  await inTree(page, name).click();
  const disclosure = inTree(page, name).getByRole('button', { name: /^(expand|collapse) / });
  if ((await disclosure.getAttribute('aria-expanded')) !== 'true') {
    await disclosure.click();
  }
  await expect(disclosure).toHaveAttribute('aria-expanded', 'true');
}

/**
 * One row of the tree, by the name it shows.
 *
 * By the row's own `title`, which the explorer puts the file's name in so that
 * a name too long for the column can still be read. It is the whole name and
 * not part of one, which is what makes `img-00001.jpg` a different row from
 * `img-00011.jpg`.
 *
 * It finds the row only while the tree draws it, which is while it is on show
 * or a few dozen rows from it; [`expectListed`](#expectlisted) and
 * [`expectNotListed`](#expectnotlisted) scroll the tree to ask after a row
 * wherever it stands.
 */
export function row(page: Page, name: string): Locator {
  return page.locator(`tbody tr[title="${name}"]`);
}

/**
 * The rows of the files named, each found by its whole name: what a folder
 * holds, counted without the folders and notes the tree draws around it.
 */
export function rowsNamed(page: Page, names: readonly string[]): Locator {
  return page.locator(names.map((name) => `tbody tr[title="${name}"]`).join(', '));
}

/**
 * Scrolls the tree to bring one of its rows among the rows it draws, the way a
 * person scrolls to a row before looking at it, and says whether it is in the
 * tree at all.
 *
 * The tree draws only the rows on show and a few dozen either side, so a row
 * far down a long folder — or far above the focus — is in no element until the
 * list is scrolled to it, and a locator for it finds nothing whether the row is
 * listed or not. This scrolls through the list a screenful at a time from the
 * top, each step waited on until every row on show is drawn, and stops where
 * `target` is drawn, leaving it on the screen. Where it is not drawn anywhere,
 * the list is put back where it was.
 */
export async function scrollTreeTo(page: Page, target: Locator): Promise<boolean> {
  if ((await target.count()) > 0) {
    await target.first().scrollIntoViewIfNeeded();
    return true;
  }
  const area = tree(page);
  const start = await area.evaluate((element) => element.scrollTop);
  for (let top = 0; ; ) {
    const step = await area.evaluate(scrollDrawn, top);
    if ((await target.count()) > 0) {
      await target.first().scrollIntoViewIfNeeded();
      return true;
    }
    if (step.atEnd) {
      break;
    }
    top = step.scrollTop + step.height;
  }
  await area.evaluate(scrollDrawn, start);
  return false;
}

/**
 * Scrolls the tree's area to `top` and waits, a frame at a time, until no
 * spacer standing in for rows not drawn is on show: until the explorer has
 * drawn the rows the scroll brought on. Run in the page.
 */
async function scrollDrawn(
  area: HTMLElement,
  top: number,
): Promise<{ scrollTop: number; height: number; atEnd: boolean }> {
  area.scrollTop = top;
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  const spacerOnShow = () => {
    const shown = area.getBoundingClientRect();
    return [...area.querySelectorAll('tbody > tr[aria-hidden]')].some((spacer) => {
      const box = spacer.getBoundingClientRect();
      return box.bottom > shown.top + 1 && box.top < shown.bottom - 1;
    });
  };
  // A second or so at most: the render a scroll asks for lands within a frame
  // or two, and a spacer still on show after that is for the caller to find
  // out about through the row it is looking for.
  for (let frames = 0; frames < 60 && spacerOnShow(); frames++) {
    await frame();
  }
  await frame();
  return {
    scrollTop: area.scrollTop,
    height: area.clientHeight,
    atEnd: area.scrollTop + area.clientHeight >= area.scrollHeight - 1,
  };
}

/**
 * Waits until `target` is in the tree, scrolling the tree to it (see
 * [`scrollTreeTo`](#scrolltreeto)), and leaves it drawn: what a row being
 * listed means where only the rows on show are drawn.
 */
export async function expectListed(
  page: Page,
  target: Locator,
  { timeout = LISTED_MS }: { timeout?: number } = {},
): Promise<void> {
  await expect(async () => {
    expect(await scrollTreeTo(page, target), 'drawn somewhere in the tree').toBe(true);
  }).toPass({ timeout });
}

/**
 * Waits until `target` is nowhere in the tree: not among the rows drawn at any
 * scroll of it, rather than only among those drawn where the list stands.
 */
export async function expectNotListed(page: Page, target: Locator): Promise<void> {
  await expect(async () => {
    expect(await scrollTreeTo(page, target), 'drawn nowhere in the tree').toBe(false);
  }).toPass({ timeout: LISTED_MS });
}

/**
 * How long the two above wait, as long as an assertion of the suite does:
 * `toPass` waits without end unless told otherwise.
 */
const LISTED_MS = 30_000;

/** What the state icon says for a file this device has, fetched here or added from here. */
export const ON_THIS_DEVICE = /^On this device$/;

/** What it says while a file added here is on its way into the Library. */
export const BEING_ADDED = /^On this device, being added to the Library/;

/** What it says for a page of a book waiting for its Pack. */
export const TO_BE_PACKED = /^On this device, waiting to be packed into the Library/;

/** Every name the state icon goes by, whatever the state. */
const STATE_ICON = /^(On this device|Being fetched to this device|Fetching failed|Not fetched)/;

/**
 * What one row says its state is: the icon in the narrow column after Size,
 * whose label is its words — a laptop for a file on this device, a ring while
 * one is being fetched or added, a warning where that failed.
 *
 * A file the Library holds and this device does not draws nothing there, so
 * `remote` is this having no element at all, asserted beside the row being on
 * the screen.
 */
export function stateIcon(page: Page, name: string): Locator {
  return row(page, name).getByRole('img', { name: STATE_ICON });
}

/** That a row is on the screen and is in the Library only: no state icon. */
export async function expectRemote(page: Page, name: string): Promise<void> {
  await expectListed(page, row(page, name));
  await expect(row(page, name)).toBeVisible();
  await expect(stateIcon(page, name)).toHaveCount(0);
}

/**
 * What another device commits into the album while this device's server is
 * down, for the restart journey to find on its first listing.
 *
 * Named to sort after the album's photographs: the album is the folder the
 * journeys focus, and the details pane fetches a focused folder's cover — its
 * first image — where this device does not have it (spec: LA-33), so a name
 * sorting first would be on this device before anybody opened it. Named here
 * rather than in that journey because sorting last makes it the album's last
 * file, which the read-on journey reads off the end of.
 */
export const ARRIVED_WHILE_DOWN = 'while-the-server-was-down.jpg';

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
 * Drops files from this disk onto part of the screen, as one drop.
 *
 * Through the browser's own drag machinery rather than through an event this
 * suite builds, and that is not a detail: the explorer reads a drop with
 * `webkitGetAsEntry`, because that is the only way a dropped *folder* can be
 * walked — and a `DataTransfer` a page constructs carries no entry at all, so a
 * fabricated drop would arrive as a gesture that carried nothing. What this
 * sends is a real drag of real files, which is what a person does.
 *
 * Files, and no folder. A folder would have to be dragged in as a folder, which
 * this cannot express; the walk that reads one is unit-tested, and a folder drop
 * stays something a person checks by hand. A book therefore arrives here as its
 * pages selected together rather than as the folder holding them — which is the
 * same drop everywhere below the walk: one request, whose parts are named
 * relative to the folder they land in.
 */
export async function dropFilesOnto(
  page: Page,
  target: Locator,
  files: string[],
): Promise<void> {
  const box = await target.boundingBox();
  if (box === null) {
    throw new Error('nothing to drop onto: the target is not on the screen');
  }
  const at = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  // `dragOperationsMask: 1` is "copy", which is what dropping a file on
  // something that is not a file manager means.
  const data = { items: [], files, dragOperationsMask: 1 };
  const session = await page.context().newCDPSession(page);
  try {
    for (const type of ['dragEnter', 'dragOver', 'drop'] as const) {
      await session.send('Input.dispatchDragEvent', { type, ...at, data });
    }
  } finally {
    await session.detach();
  }
}

/** The same, for the one file a journey drops on its own. */
export function dropFileOnto(page: Page, target: Locator, file: string): Promise<void> {
  return dropFilesOnto(page, target, [file]);
}

/** Filter settings as the server keeps them for this device (spec: LA-43). */
export interface KeptFilter {
  name: string;
  recentOn: boolean;
  recent: number;
}

/**
 * Waits for the page to have kept filter settings for this device that
 * `settled` holds of (spec: LA-43): the request it sends, answered with what
 * was kept.
 *
 * A wait rather than an assertion about the wire: what the settings are is
 * asserted on the screen after a reload, and this is only what lets a journey
 * know the reload comes after the keep and not before it. Set it waiting
 * before the gesture that changes the settings.
 */
export async function filterKept(page: Page, settled: (kept: KeptFilter) => boolean): Promise<void> {
  await page.waitForResponse(async (answer) => {
    if (
      answer.request().method() !== 'POST' ||
      new URL(answer.url()).pathname !== '/api/filter-settings' ||
      !answer.ok()
    ) {
      return false;
    }
    return settled((await answer.json()) as KeptFilter);
  });
}

/**
 * The filter settings the page reads for this device as it opens (spec:
 * LA-43), or `null` where the read was refused. Set it waiting before the page
 * is opened or reloaded.
 */
async function keptFilterRead(page: Page): Promise<KeptFilter | null> {
  const answer = await page.waitForResponse(
    (one) => one.request().method() === 'GET' && new URL(one.url()).pathname === '/api/filter-settings',
  );
  return answer.ok() ? ((await answer.json()) as KeptFilter) : null;
}

/**
 * Clears the filter on screen with Clear on its line, and waits for the server
 * to have kept it cleared.
 */
export async function clearFilter(page: Page): Promise<void> {
  const status = page.getByRole('status', { name: 'filter' });
  const cleared = filterKept(page, (kept) => kept.name === '' && !kept.recentOn);
  await status.getByRole('button', { name: 'Clear' }).click();
  await cleared;
  await expect(status).toHaveCount(0);
}
