// The other device, driven from the command line.
//
// Three journeys need something the explorer cannot do to itself. Two of them
// need a Library that changes while this device is looking at it: every folder
// on the screen comes out of this device's catalog, and a catalog only moves
// when the Journal is replayed into it — so until another device commits a head,
// there is nothing for a refresh or a restart to find. The third needs the
// opposite direction: a book this device brought in, read back somewhere else,
// which is the only thing that says it is really in the Library.
//
// That other device is the one `scripts/e2e-it.sh` created the Library as, and
// what it runs is the ordinary `coffret sync` and `coffret fetch` — the commands
// a person would have typed. Nothing here reaches the server the journeys are
// driving, and nothing about it is the explorer's — it is the second device in
// the room.

import { spawn } from 'node:child_process';
import { copyFile } from 'node:fs/promises';
import path from 'node:path';

import type { Environment } from './environment';

/** How long the other device's sync has to reach MinIO and commit. */
const SYNC_TIMEOUT_MS = 120_000;

/**
 * Copies `source` into the folder the other device maps and commits it, so the
 * Library holds an Entry at `entry` that this device has never heard of.
 *
 * The Entry Path's first component is the part of the Library that device maps
 * (spec: EP-9), and what is under it is the path inside the folder — which is
 * the whole of the translation, and the reason the journeys name the Entry
 * rather than a local path they would then have to keep in step with the
 * mapping.
 */
export async function commitElsewhere(
  setting: Environment,
  entry: string,
  source: string,
): Promise<void> {
  const cut = entry.indexOf('/');
  if (cut === -1) {
    throw new Error(`${entry} names no folder inside the part of the Library that is mapped`);
  }
  await copyFile(source, path.join(setting.uploaderRoot, entry.slice(cut + 1)));
  await synced(setting);
}

/**
 * What the other device sees of one folder, having caught up and fetched it.
 *
 * The half of the round trip the browser cannot show. A folder the explorer's
 * device brought in is only really in the Library if another device can read it
 * back, and `coffret fetch` is what a person on that device would type: it
 * replays the Journal into that device's catalog (spec: CK-9) and writes the
 * files into the folder it maps.
 *
 * What comes back is the run's own summary — how many Entries it fetched, and
 * out of how many Containers. The two differ exactly where a Pack held several
 * of them (spec: PK-16), which is what says a book was packed rather than
 * carried in one Container per page.
 */
export async function fetchedElsewhere(
  setting: Environment,
  folder: string,
): Promise<{ entries: number; containers: number }> {
  const said = await ran(setting, ['fetch', '--under', folder]);
  const summary = /fetched (\d+), containers (\d+)/.exec(said);
  if (summary === null) {
    throw new Error(`the other device's fetch said nothing this can read:\n${said}`);
  }
  return { entries: Number(summary[1]), containers: Number(summary[2]) };
}

/** Runs `coffret sync` as the other device, and waits for it to commit. */
async function synced(setting: Environment): Promise<void> {
  await ran(setting, ['sync']);
}

/**
 * Runs one `coffret` command as the other device, and answers with what it said.
 *
 * The Library, the Passphrase and the state directory are the same for every one
 * of them, so they are supplied here and each caller names only its own
 * subcommand and arguments.
 *
 * A status of `2` is a run that succeeded and left findings — a sync's changed
 * file inside a Pack (spec: PK-14), a fetch's Entry it declined to place (spec:
 * EP-11) — which is an answer rather than a failure, and what the caller reads
 * is the summary. Both halves, because both commands are run through here and
 * the fetch is the one whose findings a caller actually reads.
 */
function ran(setting: Environment, command: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    const [subcommand, ...rest] = command;
    const run = spawn(
      setting.cliBinary,
      [subcommand, '--library', setting.uploaderLibrary, ...rest, '--passphrase-stdin'],
      {
        env: {
          ...process.env,
          COFFRET_STATE_DIR: setting.uploaderStateDir,
          COFFRET_LOG_DIR: setting.logDir,
        },
        stdio: ['pipe', 'pipe', 'pipe'],
      },
    );

    // What it said, kept for the failure message and for the caller that reads
    // the summary out of it. A run that worked says it in the transcript the
    // script keeps, and there is nobody watching this one.
    let said = '';
    run.stdout?.on('data', (chunk: Buffer) => (said += chunk.toString()));
    run.stderr?.on('data', (chunk: Buffer) => (said += chunk.toString()));

    const giveUp = setTimeout(() => {
      run.kill('SIGKILL');
      reject(
        new Error(
          `the other device's ${subcommand} did not finish within ${SYNC_TIMEOUT_MS}ms`,
        ),
      );
    }, SYNC_TIMEOUT_MS);

    run.on('error', (cause) => {
      clearTimeout(giveUp);
      reject(cause);
    });
    run.on('exit', (code) => {
      clearTimeout(giveUp);
      if (code === 0 || code === FINDINGS) {
        resolve(said);
        return;
      }
      reject(new Error(`the other device's ${subcommand} exited with ${code}:\n${said}`));
    });

    run.stdin?.end(`${setting.passphrase}\n`);
  });
}

/** What the command line exits with for a run that succeeded and left findings. */
const FINDINGS = 2;
