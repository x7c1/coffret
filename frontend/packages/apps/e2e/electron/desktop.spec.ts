// The desktop app, end to end: the Electron app itself, started unpackaged
// from the checkout over a Library on MinIO, and driven the way a person uses
// it.
//
// The browser journeys walk the explorer in Chromium behind the development
// host, which is the explorer but not the app. What only the app does is
// walked here: the Passphrase window served by the app's door, the explorer in
// a window of the app's own that a launch ticket in its URL admits, a drop
// taken by the paths of what was let go of rather than read into the page, the
// idle lock bringing the Passphrase window back and the unlock landing in
// place, and a quit that leaves nothing running.
//
// One journey, in order: each step stands on the one before it, and the app is
// started once.

import { execFileSync } from 'node:child_process';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';

import {
  _electron,
  expect,
  test,
  type ElectronApplication,
  type JSHandle,
  type Locator,
  type Page,
} from '@playwright/test';

import { fromEnvironment, leaf } from './environment';

const setting = fromEnvironment();

/** How long a dropped file has to reach MinIO and be committed. */
const SYNC_MS = 120_000;

/**
 * How long the idle lock is waited for once the drop is committed: the
 * interval, and as long again for the server's own clock to come round.
 */
const LOCK_MS = setting.idleMinutes * 60_000 * 2;

/**
 * How long the explorer is given to ask for the Passphrase window by itself,
 * once it has seen the lock, before the journey presses *unlock* for it. The
 * page asks on its own only while it has the focus, which a window on a
 * virtual display may never be given.
 */
const OWN_ASK_MS = 10_000;

/** How long the app is given to end once it is asked to quit. */
const QUIT_MS = 30_000;

/** One row of the current folder's listing, by the name it shows. */
function row(page: Page, name: string): Locator {
  return page.locator(`tbody tr[title="${name}"]`);
}

/** What one row says its state is. */
function chip(page: Page, name: string): Locator {
  return row(page, name).locator('span');
}

/** One folder in the tree down the left, by name. */
function inTree(page: Page, name: string): Locator {
  return page.getByRole('navigation', { name: 'folders' }).getByTitle(name, { exact: true });
}

/** A picture of where the journey has got to, for a person to look at. */
async function shot(page: Page, checkpoint: string): Promise<void> {
  await page.screenshot({ path: path.join(setting.screenshots, 'desktop', `${checkpoint}.png`) });
}

/** Whether the window `window` holds is on the screen. */
function shown(window: JSHandle): Promise<boolean> {
  return window.evaluate((held) => held.isVisible() as boolean);
}

/**
 * Drops files from this disk onto part of a window, as one drop, through the
 * browser's own drag machinery: a real drag of real files, whose `File`s carry
 * the paths the app's preload hands over.
 */
async function dropFilesOnto(page: Page, target: Locator, files: string[]): Promise<void> {
  const box = await target.boundingBox();
  if (box === null) {
    throw new Error('nothing to drop onto: the target is not on the screen');
  }
  const at = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  // `dragOperationsMask: 1` is "copy".
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

/**
 * Whether the app's log says the server locked the Library for being idle.
 *
 * The lock is the server's own doing, on its own clock, and nothing on the
 * screen changes when it lands: the explorer learns of it from the next thing
 * it asks. So the journey reads the event the server records as it locks, and
 * only then asks.
 */
async function lockedForIdling(): Promise<boolean> {
  let names: string[];
  try {
    names = await readdir(setting.logDir);
  } catch {
    return false;
  }
  for (const name of names) {
    let text: string;
    try {
      text = await readFile(path.join(setting.logDir, name), 'utf8');
    } catch {
      continue;
    }
    if (text.split('\n').some((line) => line.includes('"operation":"lock"') && line.includes('"how":"idle"'))) {
      return true;
    }
  }
  return false;
}

/**
 * The processes still running that were started for this run's app: every
 * one of Electron's carries the browser-data directory the journey gave it.
 */
function leftRunning(): string[] {
  try {
    return execFileSync('pgrep', ['-f', '--', setting.userDataDir], { encoding: 'utf8' })
      .split('\n')
      .filter((line) => line !== '');
  } catch {
    // pgrep exits 1 where nothing matched.
    return [];
  }
}

/** The environment the app is started with: this one, and the run's own. */
function appEnvironment(): Record<string, string> {
  const inherited = Object.entries(process.env).filter(
    (entry): entry is [string, string] => entry[1] !== undefined,
  );
  return {
    ...Object.fromEntries(inherited),
    COFFRET_STATE_DIR: setting.stateDir,
    COFFRET_LOG_DIR: setting.logDir,
    COFFRET_IDLE_MINUTES: String(setting.idleMinutes),
  };
}

test('open a Library, drop a file onto it, unlock it in place after the idle lock, and quit', async () => {
  const app: ElectronApplication = await _electron.launch({
    executablePath: setting.electronBinary,
    args: [setting.appDir, `--user-data-dir=${setting.userDataDir}`, ...setting.electronSwitches],
    env: appEnvironment(),
  });
  let quit = false;
  try {
    // The first window is the Passphrase window, served by the app's door:
    // the Libraries on this device, and the Passphrase of the one chosen.
    const door = await app.firstWindow();
    const doorWindow = await app.browserWindow(door);
    await door.locator('#library').selectOption({ label: setting.library });
    await door.locator('#passphrase').fill(setting.passphrase);
    await shot(door, '01-passphrase');
    const opened = app.waitForEvent('window');
    await door.locator('#open').click();

    // The explorer comes in a window of its own, admitted by the ticket in the
    // URL it was loaded from, which the page takes out of the address; and the
    // Passphrase window goes.
    const explorer = await opened;
    await expect(explorer.locator('footer').getByText(setting.library, { exact: true })).toBeVisible();
    expect(new URL(explorer.url()).hash).toBe('');
    await expect.poll(() => shown(doorWindow)).toBe(false);

    // It lists the Library: the mapped folder at the root, and the album in it.
    await expect(row(explorer, setting.mapped)).toBeVisible();
    await row(explorer, setting.mapped).dblclick();
    await row(explorer, leaf(setting.album)).dblclick();
    await expect(row(explorer, setting.firstPhoto)).toBeVisible();
    await shot(explorer, '02-listed');

    // A file dropped onto the album from this disk: handed to the app by its
    // path, confirmed, and carried into the Library.
    const dropped = path.basename(setting.dropFile);
    await expect(row(explorer, dropped)).toHaveCount(0);
    await dropFilesOnto(explorer, row(explorer, setting.firstPhoto), [setting.dropFile]);
    const question = explorer.getByRole('dialog');
    await expect(question).toContainText('1 file');
    await question.getByRole('button', { name: 'Add', exact: true }).click();
    await expect(row(explorer, dropped)).toHaveCount(1);
    await expect(chip(explorer, dropped)).toHaveText('present', { timeout: SYNC_MS });
    await shot(explorer, '03-dropped');

    // Left alone for the idle interval, the server locks the Library. The next
    // thing the explorer asks is refused as locked, and the Passphrase window
    // comes back to take the Passphrase: asked for by the page itself, or by
    // *unlock* where the page did not have the focus to ask.
    await expect.poll(lockedForIdling, { timeout: LOCK_MS, intervals: [5_000] }).toBe(true);
    await inTree(explorer, setting.mapped).click();
    try {
      await expect.poll(() => shown(doorWindow), { timeout: OWN_ASK_MS }).toBe(true);
    } catch {
      await explorer.getByRole('button', { name: 'unlock', exact: true }).first().click();
      await expect.poll(() => shown(doorWindow)).toBe(true);
    }
    // The one Library served, listed under its name and chosen by its ID.
    await expect(door.locator('#library option:checked')).toHaveText(setting.library);
    await shot(door, '04-unlock');

    // Unlocked in place: the window goes, and the explorer that was open is
    // the one that answers again, without being loaded afresh.
    await door.locator('#passphrase').fill(setting.passphrase);
    await door.locator('#open').click();
    await expect.poll(() => shown(doorWindow)).toBe(false);
    await expect(row(explorer, leaf(setting.album))).toBeVisible();
    await row(explorer, leaf(setting.album)).dblclick();
    await expect(chip(explorer, dropped)).toHaveText('present');
    await shot(explorer, '05-unlocked');

    // Quitting ends the app, and with it everything it started.
    const child = app.process();
    const exited = new Promise<number | null>((resolve) => {
      if (child.exitCode !== null) {
        resolve(child.exitCode);
      } else {
        child.once('exit', (code) => resolve(code));
      }
    });
    await app.evaluate(({ app: electron }) => electron.quit());
    quit = true;
    const code = await Promise.race([
      exited,
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error(`the app did not end within ${QUIT_MS}ms of being asked to quit`)), QUIT_MS),
      ),
    ]);
    expect(code).toBe(0);
    await expect.poll(leftRunning).toEqual([]);
  } finally {
    if (!quit) {
      await app.close().catch(() => {});
    }
  }
});
