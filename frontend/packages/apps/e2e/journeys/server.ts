// The `coffret-server` process the journeys are driven against.
//
// It belongs to the suite rather than to the script that starts the suite, and
// for one reason: the outage journey kills it and starts it again, which a
// process the script held would be out of reach of. Everything else about it is
// the script's — the binary, the state directory, the Library's name, the port,
// and the credentials the Library's Storage is reached with, all of which
// arrive through the environment.
//
// One port, kept across a restart. The explorer is served by `vite preview`
// which is aimed at the server once, when it starts, so a server that came back
// on a port the operating system chose would be a server the page could no
// longer reach — and the outage journey would be proving the wrong thing.

import { spawn, type ChildProcess } from 'node:child_process';
import { createWriteStream, type WriteStream } from 'node:fs';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

import type { Environment } from './environment';

/** How long a server gets to open the Library and answer, before giving up. */
const STARTUP_TIMEOUT_MS = 60_000;

/** How long a killed server gets to stop answering. */
const SHUTDOWN_TIMEOUT_MS = 10_000;

/** How often either of those is asked about. */
const POLL_MS = 100;

/**
 * One `coffret-server`, startable and killable more than once.
 *
 * Killed rather than asked to stop: what the outage journey stands for is the
 * server going away, and a process given the chance to close its files tidily
 * is the easy half of that. What has to still work afterwards is the Library on
 * disk, which the next start opens again.
 */
export class CoffretServer {
  private readonly environment: Environment;
  private running: ChildProcess | null = null;
  private log: WriteStream | null = null;
  /** Why the process ended, where it ended on its own. */
  private died: string | null = null;

  constructor(environment: Environment) {
    this.environment = environment;
  }

  /** Where a page reaches it, which is what the vite proxy is aimed at. */
  get url(): string {
    return `http://127.0.0.1:${this.environment.serverPort}`;
  }

  /**
   * Starts it and waits until it answers.
   *
   * The Passphrase goes in over standard input and the input is closed behind
   * it: one process is one unlock, and there is nothing else the server ever
   * reads from there.
   */
  async start(): Promise<void> {
    if (this.running !== null) {
      throw new Error('the server is already running');
    }
    await mkdir(this.environment.logDir, { recursive: true });
    const log = createWriteStream(path.join(this.environment.logDir, 'coffret-server.log'), {
      flags: 'a',
    });
    const started = spawn(
      this.environment.serverBinary,
      [
        '--library',
        this.environment.library,
        '--passphrase-stdin',
        '--port',
        String(this.environment.serverPort),
      ],
      {
        env: {
          ...process.env,
          COFFRET_STATE_DIR: this.environment.stateDir,
          COFFRET_LOG_DIR: this.environment.logDir,
        },
        stdio: ['pipe', 'pipe', 'pipe'],
      },
    );
    started.stdout?.pipe(log, { end: false });
    started.stderr?.pipe(log, { end: false });
    this.died = null;
    started.on('exit', (code, signal) => {
      this.died = `it exited with ${signal ?? `status ${code}`}`;
    });
    started.stdin?.end(`${this.environment.passphrase}\n`);

    this.running = started;
    this.log = log;
    try {
      await this.answering();
    } catch (reason) {
      // A server that was spawned and never answered is still a process, and
      // the fixture that started it never reaches its own teardown when `start`
      // throws. So it goes with the failure that reported it: left alone it
      // outlives the run holding this port, and the next run's first stage
      // would find it answering there instead of the server it just started —
      // over a state directory that run had already deleted.
      await this.stop().catch(() => {
        // Whatever went wrong stopping it, the failure worth reporting is the
        // one that got here.
      });
      throw reason;
    }
  }

  /**
   * Kills it and waits until nothing answers at its port.
   *
   * Idempotent, because it is called both by the outage journey and by the
   * teardown that runs after every journey — including the one that has already
   * killed it and started it again.
   */
  async stop(): Promise<void> {
    const running = this.running;
    if (running === null) {
      return;
    }
    this.running = null;
    const ended = new Promise<void>((resolve) => {
      if (running.exitCode !== null || running.signalCode !== null) {
        resolve();
        return;
      }
      running.once('exit', () => resolve());
    });
    running.kill('SIGKILL');
    await ended;
    this.log?.end();
    this.log = null;
    await this.silent();
  }

  /** Waits until the Library is open and the routes answer. */
  private async answering(): Promise<void> {
    const until = Date.now() + STARTUP_TIMEOUT_MS;
    for (;;) {
      if (this.died !== null) {
        throw new Error(
          `the server did not start: ${this.died}. What it said is in ${this.environment.logDir}.`,
        );
      }
      if (await this.answers()) {
        return;
      }
      if (Date.now() > until) {
        throw new Error(
          `the server did not answer at ${this.url} within ${STARTUP_TIMEOUT_MS}ms. ` +
            `What it said is in ${this.environment.logDir}.`,
        );
      }
      await pause(POLL_MS);
    }
  }

  /** Waits until nothing answers there any more. */
  private async silent(): Promise<void> {
    const until = Date.now() + SHUTDOWN_TIMEOUT_MS;
    while (await this.answers()) {
      if (Date.now() > until) {
        throw new Error(`something is still answering at ${this.url} after the server was killed`);
      }
      await pause(POLL_MS);
    }
  }

  /** Whether the routes answer right now. */
  private async answers(): Promise<boolean> {
    try {
      const response = await fetch(`${this.url}/api/library`);
      return response.ok;
    } catch {
      // Nothing is listening, or the connection was refused part way. Either is
      // the answer this is asking for and neither is a failure of the suite —
      // this is the poll that decides when the server is up and when it is gone.
      return false;
    }
  }
}

function pause(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
