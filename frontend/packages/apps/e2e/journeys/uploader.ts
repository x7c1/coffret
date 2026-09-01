// The other device, driven from the command line.
//
// Two journeys need something the explorer cannot do to itself: a Library that
// changes while this device is looking at it. Every folder on the screen comes
// out of this device's catalog, and a catalog only moves when the Journal is
// replayed into it — so until another device commits a head, there is nothing
// for a refresh or a restart to find.
//
// That other device is the one `scripts/e2e-it.sh` created the Library as, and
// what it runs is the ordinary `coffret sync`: a file copied into the folder it
// maps, and the command a person would have typed. Nothing here reaches the
// server the journeys are driving, and nothing about it is the explorer's — it
// is the second device in the room.

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

/** Runs `coffret sync` as the other device, and waits for it to commit. */
function synced(setting: Environment): Promise<void> {
  return new Promise((resolve, reject) => {
    const run = spawn(
      setting.cliBinary,
      ['sync', '--library', setting.uploaderLibrary, '--passphrase-stdin'],
      {
        env: {
          ...process.env,
          COFFRET_STATE_DIR: setting.uploaderStateDir,
          COFFRET_LOG_DIR: setting.logDir,
        },
        stdio: ['pipe', 'pipe', 'pipe'],
      },
    );

    // What it said, kept for the failure message. A run that worked says it in
    // the transcript the script keeps, and there is nobody watching this one.
    let said = '';
    run.stdout?.on('data', (chunk: Buffer) => (said += chunk.toString()));
    run.stderr?.on('data', (chunk: Buffer) => (said += chunk.toString()));

    const giveUp = setTimeout(() => {
      run.kill('SIGKILL');
      reject(new Error(`the other device's sync did not finish within ${SYNC_TIMEOUT_MS}ms`));
    }, SYNC_TIMEOUT_MS);

    run.on('error', (cause) => {
      clearTimeout(giveUp);
      reject(cause);
    });
    run.on('exit', (code) => {
      clearTimeout(giveUp);
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`the other device's sync exited with ${code}:\n${said}`));
    });

    run.stdin?.end(`${setting.passphrase}\n`);
  });
}
