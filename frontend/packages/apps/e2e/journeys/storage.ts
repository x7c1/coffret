// The Library's Storage, as the journeys that take it away reach it.
//
// MinIO in the container `scripts/e2e-it.sh` started. The script owns it — it
// made it, and its teardown removes it — so what is here is only the moves a
// journey makes on it: stopping it, so that what the server asks of Storage
// meets a refused connection the way it would meet an outage, and starting it
// again, so that the journeys after it run against a Storage that is up; and
// pausing it, so that what the server asks waits, and unpausing it.
//
// Stopped rather than paused. A paused container holds its port and answers
// nothing, which the server would sit out for as long as its client's timeouts
// say; a stopped one refuses the connection, which is the same outage said at
// once.

import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

import type { Environment } from './environment';

const run = promisify(execFile);

/** How long a started MinIO gets to answer its health check. */
const STARTUP_TIMEOUT_MS = 60_000;

/** How often it is asked. */
const POLL_MS = 250;

/** Stops the container Storage runs in. */
export async function stopStorage(environment: Environment): Promise<void> {
  await run('docker', ['stop', '--time', '5', environment.minioContainer]);
}

/**
 * Starts it again and waits until it answers.
 *
 * The same container, so the same data and the same port: the Library the
 * server holds is still there, and the server is still aimed at it.
 */
export async function startStorage(environment: Environment): Promise<void> {
  await run('docker', ['start', environment.minioContainer]);
  const until = Date.now() + STARTUP_TIMEOUT_MS;
  for (;;) {
    if (await live(environment)) {
      return;
    }
    if (Date.now() > until) {
      throw new Error(
        `MinIO in ${environment.minioContainer} did not answer at ${environment.minioUrl} ` +
          `within ${STARTUP_TIMEOUT_MS}ms of being started again`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/**
 * Pauses the container Storage runs in: it holds its port and answers
 * nothing, so what the server asks of Storage waits — which is how a journey
 * holds a run under way, before its commit, for as long as it needs to.
 */
export async function pauseStorage(environment: Environment): Promise<void> {
  await run('docker', ['pause', environment.minioContainer]);
}

/** Lets a paused Storage answer again, and waits until it does. */
export async function unpauseStorage(environment: Environment): Promise<void> {
  await run('docker', ['unpause', environment.minioContainer]);
  const until = Date.now() + STARTUP_TIMEOUT_MS;
  while (!(await live(environment))) {
    if (Date.now() > until) {
      throw new Error(
        `MinIO in ${environment.minioContainer} did not answer at ${environment.minioUrl} ` +
          `within ${STARTUP_TIMEOUT_MS}ms of being unpaused`,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/** Whether MinIO says it is live. */
async function live(environment: Environment): Promise<boolean> {
  try {
    const response = await fetch(`${environment.minioUrl}/minio/health/live`);
    return response.ok;
  } catch {
    // Nothing is listening yet, which is what this poll is waiting out.
    return false;
  }
}
