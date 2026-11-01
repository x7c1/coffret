// Which launch file a journey opens, over a directory standing in for the
// one the explorer's host writes into.

import { mkdtemp, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { nextLaunch, serverStarting } from './launch';

let dir: string;

beforeEach(async () => {
  dir = await mkdtemp(path.join(tmpdir(), 'coffret-launch-'));
});

afterEach(async () => {
  await rm(dir, { recursive: true, force: true });
});

/** Writes a launch file as the host does: under another name, renamed into place. */
async function write(ticket: string, instance: string): Promise<void> {
  const staged = path.join(dir, '.explorer.html.tmp');
  await writeFile(
    staged,
    '<!doctype html>\n' +
      `<meta name="coffret-instance-id" content="${instance}">\n` +
      `<meta http-equiv="refresh" content="0; url=http://127.0.0.1:1/#launch=${ticket}">\n`,
  );
  await rename(staged, path.join(dir, 'explorer.html'));
}

function later(ms: number, then: () => Promise<void>): Promise<void> {
  return new Promise((resolve, reject) => {
    setTimeout(() => then().then(resolve, reject), ms);
  });
}

describe('nextLaunch', () => {
  it("opens the first file of this worker's server", async () => {
    await serverStarting(dir);
    const [launch] = await Promise.all([
      nextLaunch(dir, { afterRestart: false, timeoutMs: 5_000 }),
      later(200, () => write('first', 'gen-1')),
    ]);
    expect(launch.ticket).toBe('first');
  });

  it("passes over the file of the last worker's server and waits for the running one's", async () => {
    // What a worker started after a failed journey finds: the file the last
    // worker's server was given, not yet replaced.
    await write('spent', 'gen-1');
    await serverStarting(dir);
    const [launch] = await Promise.all([
      nextLaunch(dir, { afterRestart: false, timeoutMs: 5_000 }),
      later(300, () => write('fresh', 'gen-2')),
    ]);
    expect(launch).toMatchObject({ ticket: 'fresh', instance: 'gen-2' });
  });

  it('gives up on a directory holding only the file of the server before', async () => {
    await write('spent', 'gen-1');
    await serverStarting(dir);
    await expect(nextLaunch(dir, { afterRestart: false, timeoutMs: 300 })).rejects.toThrow(
      /no fresh launch file appeared/,
    );
  });

  it('takes the next ticket of one generation, and only a new generation after a restart', async () => {
    await serverStarting(dir);
    await write('one', 'gen-1');
    expect((await nextLaunch(dir, { afterRestart: false, timeoutMs: 1_000 })).ticket).toBe('one');

    await write('two', 'gen-1');
    expect((await nextLaunch(dir, { afterRestart: false, timeoutMs: 1_000 })).ticket).toBe('two');

    await write('three', 'gen-1');
    const [launch] = await Promise.all([
      nextLaunch(dir, { afterRestart: true, timeoutMs: 5_000 }),
      later(300, () => write('four', 'gen-2')),
    ]);
    expect(launch).toMatchObject({ ticket: 'four', instance: 'gen-2' });
  });
});
