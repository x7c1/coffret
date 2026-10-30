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
  type Request,
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

/**
 * What one row says its state is: the icon in the narrow column after Size,
 * whose label is its words. A file the Library holds and this device does not
 * draws none.
 */
function stateIcon(page: Page, name: string): Locator {
  return row(page, name).getByRole('img', {
    name: /^(On this device|Being fetched to this device|Fetching failed|Not fetched)/,
  });
}

/** What the state icon says for a file this device has. */
const ON_THIS_DEVICE = /^On this device$/;

/** One folder's disclosure button in the tree, which opens and closes it. */
function disclosure(page: Page, name: string): Locator {
  return page
    .getByRole('tree', { name: 'Library' })
    .locator(`tr[title="${name}"]`)
    .getByRole('button', { name: /^(expand|collapse) / });
}

/** Opens one folder in the tree, drawing its rows under it. */
async function expand(page: Page, name: string): Promise<void> {
  await page.getByRole('button', { name: `expand ${name}`, exact: true }).click();
  await expect(disclosure(page, name)).toHaveAttribute('aria-expanded', 'true');
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
 * What the explorer asks on its own that is not use (spec: DK-4): what the
 * server is doing, the remote head, the Library's name and the host's session.
 * Everything else under `/api/` needs the Library's keys, and so puts the idle
 * lock off.
 */
const UNATTENDED = new Set(['/api/work', '/api/remote-head', '/api/library', '/auth/session']);

/**
 * How long the explorer has to have asked nothing before the quiet begins:
 * several of the work route's poll intervals (700 ms), so that the poll
 * following the drop's sync has heard it end, and the reloads that ending
 * asks for have been asked.
 */
const SETTLE_MS = 3_000;

/** The explorer's requests, counted while out and, once watched, kept where they are use. */
interface Asking {
  /** Resolves once nothing the explorer asked is still out, and it has asked nothing for {@link SETTLE_MS}. */
  settled(): Promise<void>;
  /** Starts keeping every request from here on that is use. */
  watch(): void;
  /** What was asked since {@link watch} that is use, as `<ISO time> METHOD path?query`. */
  used(): string[];
  /** Stops listening. */
  stop(): void;
}

function asking(page: Page): Asking {
  const out = new Set<Request>();
  const used: string[] = [];
  let last = Date.now();
  let watching = false;
  const sent = (request: Request) => {
    out.add(request);
    last = Date.now();
    const { pathname, search } = new URL(request.url());
    const own = pathname.startsWith('/api/') || pathname.startsWith('/auth/');
    if (watching && own && !UNATTENDED.has(pathname)) {
      used.push(`${new Date().toISOString()} ${request.method()} ${pathname}${search}`);
    }
  };
  const ended = (request: Request) => {
    out.delete(request);
    last = Date.now();
  };
  page.on('request', sent);
  page.on('requestfinished', ended);
  page.on('requestfailed', ended);
  return {
    settled: () =>
      expect.poll(() => out.size === 0 && Date.now() - last >= SETTLE_MS, { intervals: [250] }).toBe(true),
    watch: () => {
      watching = true;
    },
    used: () => [...used],
    stop: () => {
      page.off('request', sent);
      page.off('requestfinished', ended);
      page.off('requestfailed', ended);
    },
  };
}

/** One event from the app's log, as much of it as the idle wait reads. */
interface Logged {
  timestamp: string;
  fields: { operation?: string; how?: string; holds?: number; idle_ms?: number };
}

/** Every event the app has logged so far. */
async function logged(): Promise<Logged[]> {
  let names: string[];
  try {
    names = await readdir(setting.logDir);
  } catch {
    return [];
  }
  const events: Logged[] = [];
  for (const name of names) {
    let text: string;
    try {
      text = await readFile(path.join(setting.logDir, name), 'utf8');
    } catch {
      continue;
    }
    for (const line of text.split('\n')) {
      try {
        events.push(JSON.parse(line) as Logged);
      } catch {
        // A line still being written, or the empty one after the last.
      }
    }
  }
  return events;
}

/**
 * Waits for the server to lock the Library for being idle, the quiet having
 * begun at `quietFrom` (milliseconds since the epoch), and fails at once where
 * something broke that quiet.
 *
 * The lock is the server's own doing, on its own clock, and nothing on the
 * screen changes when it lands: the explorer learns of it from the next thing
 * it asks. So the journey reads the event the server records as it locks, and
 * only then asks.
 *
 * Nothing the journey or the page does may put it off, so whatever did is the
 * failure, said when it happens rather than waited out: a request of the
 * explorer's that is use, or the watcher waking at the end of the interval to
 * find the Library wanted since the quiet began — still held by a piece of
 * work, or let go of later — which it logs (at `debug`) as it puts the lock
 * off.
 */
async function lockedForIdling(asked: Asking, quietFrom: number): Promise<void> {
  const deadline = Date.now() + LOCK_MS;
  for (;;) {
    const events = (await logged()).filter(
      (event) => event.fields.operation === 'lock' && Date.parse(event.timestamp) >= quietFrom,
    );
    if (events.some((event) => event.fields.how === 'idle')) {
      return;
    }
    const used = asked.used();
    const putOff = events.filter((event) => {
      if (event.fields.how !== 'put_off') {
        return false;
      }
      const lastUse = Date.parse(event.timestamp) - (event.fields.idle_ms ?? 0);
      return (event.fields.holds ?? 0) > 0 || lastUse > quietFrom;
    });
    if (used.length > 0 || putOff.length > 0) {
      throw new Error(
        'the idle lock was put off during the quiet the journey waits it out in: ' +
          JSON.stringify({ explorerAsked: used, serverPutOff: putOff }),
      );
    }
    if (Date.now() > deadline) {
      throw new Error(`the Library was not locked within ${LOCK_MS}ms of being left alone, and nothing said why`);
    }
    await new Promise((resolve) => setTimeout(resolve, 1_000));
  }
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
    // Where the idle lock is put off is logged at debug, and it is what says
    // why a wait for the lock did not end.
    COFFRET_LOG: 'debug',
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
    const asked = asking(explorer);
    await expect(explorer.getByRole('banner').getByText(setting.library, { exact: true })).toBeVisible();
    expect(new URL(explorer.url()).hash).toBe('');
    await expect.poll(() => shown(doorWindow)).toBe(false);

    // It lists the Library: the mapped folder at the root, and the album in it.
    await expect(row(explorer, setting.mapped)).toBeVisible();
    await expand(explorer, setting.mapped);
    await expand(explorer, leaf(setting.album));
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
    await expect(stateIcon(explorer, dropped)).toHaveAccessibleName(ON_THIS_DEVICE, { timeout: SYNC_MS });
    await shot(explorer, '03-dropped');

    // Left alone for the idle interval, the server locks the Library. The next
    // thing the explorer asks is refused as locked, and the Passphrase window
    // comes back to take the Passphrase: asked for by the page itself, or by
    // *unlock* where the page did not have the focus to ask.
    // The mapped folder closed and then focused is the folder the details pane
    // describes, its listing asked for afresh: the next thing the explorer
    // asks. The refusal is the lock heard, and the tree gives way to the
    // refusal at once, so nothing more is pressed in it until the unlock.
    // The quiet begins once the explorer has had every answer it asked for
    // after the drop committed, and has stopped asking; from there nothing
    // may put the lock off, and whatever does is said as it happens.
    await asked.settled();
    const quietFrom = Date.now();
    asked.watch();
    await lockedForIdling(asked, quietFrom);
    asked.stop();
    await explorer.getByRole('button', { name: `collapse ${setting.mapped}`, exact: true }).click();
    await row(explorer, setting.mapped).click();
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
    // the one that answers again, without being loaded afresh — the mapped
    // folder still closed, as it was left.
    await door.locator('#passphrase').fill(setting.passphrase);
    await door.locator('#open').click();
    await expect.poll(() => shown(doorWindow)).toBe(false);
    await expand(explorer, setting.mapped);
    await expect(row(explorer, leaf(setting.album))).toBeVisible();
    await expand(explorer, leaf(setting.album));
    await expect(stateIcon(explorer, dropped)).toHaveAccessibleName(ON_THIS_DEVICE);
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
