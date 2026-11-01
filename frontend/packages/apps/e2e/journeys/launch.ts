// The launch file the explorer's host writes, as a journey opens it.
//
// The host admits a browser only by a browser token, and a page gets one by
// exchanging the one-time ticket in a file only this account can read
// (spec: LA-21). So a journey opens the explorer exactly the way a person does: it reads
// the file the way any process of the owning account can, and opens it in the
// browser, which follows the file's refresh to the explorer.
//
// The host keeps one file ready: written again once its ticket was used, and
// afresh, for the next generation, when the server behind it is started again.
// Which file is the next one to open is read off the file itself — the ticket
// in it, and the generation it names — rather than off when it was written.
//
// Each worker starts a server of its own, and Playwright starts a new worker
// after a journey fails. When the new server starts, the file standing is the
// one written for the server before it, whose ticket the restart ended; it
// stays until the host writes the file for the new server. So the generation
// standing at a server's start is passed over (see [`serverStarting`]), and
// the new worker's first journey waits for the running server's file rather
// than failing on a ticket spent before it.

import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type { Page } from '@playwright/test';

/** The name the host writes the file under, in its directory. */
const LAUNCH_FILE = 'explorer.html';

/** How long a fresh file is waited for. */
const FRESH_TIMEOUT_MS = 30_000;

/** How often it is looked for. */
const POLL_MS = 100;

/** One launch file, read. */
export interface Launch {
  file: string;
  ticket: string;
  /** The host's generation the ticket was issued in, which is public. */
  instance: string;
}

/** What the last file a journey opened carried. */
let opened: Launch | null = null;

/**
 * The generation of the file that stood in the directory when this worker's
 * server was started — one written for a server before it — or `null` where
 * none stood.
 */
let retired: string | null = null;

/**
 * Notes, just before this worker starts its server, the generation of the
 * launch file standing in `dir`, so that no journey opens it.
 *
 * Called before the server starts rather than after: once it answers, the host
 * may write the file for it at any moment, and that one is to be opened.
 */
export async function serverStarting(dir: string): Promise<void> {
  opened = null;
  retired = (await read(path.join(dir, LAUNCH_FILE)))?.instance ?? null;
}

/**
 * Opens the next launch file in `page` (see [`nextLaunch`]).
 */
export async function openLaunchFile(
  page: Page,
  dir: string,
  { afterRestart }: { afterRestart: boolean },
): Promise<void> {
  const launch = await nextLaunch(dir, { afterRestart });
  await page.goto(pathToFileURL(launch.file).href);
}

/**
 * Waits for the next launch file in `dir` and takes it as opened: one whose
 * ticket no journey has opened yet, whose generation is not the one standing
 * when this worker's server started, and — `afterRestart` — whose generation
 * is not the one the last file was of either.
 */
export async function nextLaunch(
  dir: string,
  { afterRestart, timeoutMs = FRESH_TIMEOUT_MS }: { afterRestart: boolean; timeoutMs?: number },
): Promise<Launch> {
  const before = opened;
  const launch = await fresh(
    path.join(dir, LAUNCH_FILE),
    (found) =>
      found.instance !== retired &&
      found.ticket !== before?.ticket &&
      (!afterRestart || found.instance !== before?.instance),
    timeoutMs,
  );
  opened = launch;
  return launch;
}

/** The first launch file at `file` that `wanted` takes. */
async function fresh(
  file: string,
  wanted: (found: Launch) => boolean,
  timeoutMs: number,
): Promise<Launch> {
  const until = Date.now() + timeoutMs;
  for (;;) {
    const found = await read(file);
    if (found !== null && wanted(found)) {
      return found;
    }
    if (Date.now() > until) {
      throw new Error(
        `no fresh launch file appeared at ${file} within ${timeoutMs}ms. What the ` +
          'explorer host said is in its output above and in the logs directory.',
      );
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/** The launch file at `file`, or `null` where none stands there yet. */
async function read(file: string): Promise<Launch | null> {
  let page: string;
  try {
    page = await readFile(file, 'utf8');
  } catch {
    // Not written yet, or between one file and the next: the host removes a
    // spent one before it writes the one after.
    return null;
  }
  const ticket = /#launch=([A-Za-z0-9_-]+)/.exec(page)?.[1];
  const instance = /name="coffret-instance-id" content="([^"]+)"/.exec(page)?.[1];
  return ticket === undefined || instance === undefined ? null : { file, ticket, instance };
}
