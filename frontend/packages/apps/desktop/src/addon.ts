// The Node-API addon the app runs the server in: coffret-napi, built by
// `make napi-addon` as backend/target/release/coffret.node with the explorer
// compiled in, and shipped beside a packaged app's code (`resources.ts`). This
// module says what it exports, and loads it.

import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';

import type { Resources } from './resources.js';

/** What a Passphrase window is opened to do. */
export type DoorMode = 'Open' | 'Unlock';

/**
 * What the addon tells the app. Each is a kind and nothing else: none carries
 * a token, a ticket or a URL, which the app asks for through a call of its
 * own.
 */
export type DesktopEvent =
  | 'LibraryOpened'
  | 'LibraryUnlocked'
  | 'UnlockRequested'
  /**
   * The door ended a Passphrase window's token after its last refused try:
   * that window can take no Passphrase any more, and is loaded afresh the
   * next time it is brought forward. Which window is not said, since that
   * would take its token; the app keeps one.
   */
  | 'PassphraseWindowSpent'
  | 'ServerStopped';

/** One Library on this device, as the addon lists it. */
export interface ListedLibrary {
  /** Its ID, in the 16 lowercase hex characters it is spelled in. */
  id: string;
  /** Its name, or null where nothing has named it yet. */
  name: string | null;
  /** Whether more than one directory on this device holds it; opening it is refused until one is gone. */
  needsAttention: boolean;
}

/** The Library served, as the addon tells of it. */
export interface ServedLibrary {
  /** Its name, or null where nothing has named it yet. */
  name: string | null;
  /** Whether it is unlocked, or has locked itself and waits for the Passphrase. */
  unlocked: boolean;
}

/** One file a drop took, as the explorer's page is told of it. */
export interface DroppedFileRow {
  /** Its path relative to the folder it was dropped from. */
  path: string;
  /** Its length in bytes. */
  size: number;
  /** Its modification time, in milliseconds from the Unix epoch. */
  mtime: number;
}

/** One thing a drop met and did not take, and why, in one word. */
export interface NotTakenRow {
  path: string;
  reason: string;
}

/**
 * What a drop is answered with: the one-time handle the explorer's import
 * names, or `null` where nothing was taken, and what each file is. No path on
 * this device is in it.
 */
export interface DropAnswer {
  handle: string | null;
  files: DroppedFileRow[];
  notTaken: NotTakenRow[];
}

/**
 * The addon's exports. Every one throws, as an `Error` whose message is the
 * sentence to show, where the addon refuses.
 */
export interface Addon {
  /**
   * Starts the log, the server's runtime and the Passphrase door. `idleMinutes`
   * is what `COFFRET_IDLE_MINUTES` was set to, if anything.
   */
  start(identifier: string, idleMinutes?: string | null): void;
  /** The Libraries on this device, each by its ID and the name it is listed under. */
  libraries(): ListedLibrary[];
  /**
   * The URL to load one Passphrase window from, opened in `mode`, with a fresh
   * door token in its fragment: for the window and nothing else, a log
   * included.
   */
  openDoor(mode: DoorMode): string;
  /** The Library served, if one is open and its server has not stopped. */
  served(): ServedLibrary | null;
  /** A one-time launch ticket for the explorer, good for one exchange within a minute. */
  issueTicket(): string;
  /** Where the explorer is served, as `127.0.0.1:<port>`, if a Library is open. */
  explorerAddress(): string | null;
  /**
   * Walks what a drop onto the explorer's window let go of, given as the whole
   * path of each item, and holds what it took for the explorer's import.
   */
  takeDrop(paths: string[]): Promise<DropAnswer>;
  /** Hands every event from now on to `callback`. */
  listen(callback: (event: DesktopEvent) => void): void;
  /** Discards every browser session, launch ticket and door token. */
  retire(): void;
}

/** The addon, loaded from where `resources` says it is. */
export function loadAddon(resources: Resources): Addon {
  const path = resources.addon;
  if (!existsSync(path)) {
    throw new Error(`The Node-API addon is not at ${path}; ${resources.missingAddon}`);
  }
  return createRequire(import.meta.url)(path) as Addon;
}
