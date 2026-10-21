// Whether the desktop shows a tray icon at all.
//
// macOS and Windows always do. On Linux a tray icon is a StatusNotifierItem,
// which is shown only where something on the session bus hosts them — owns
// `org.kde.StatusNotifierWatcher`. GNOME has no such host of its own (an
// extension adds one), and an icon nothing hosts is made without complaint and
// shown nowhere, which would leave the app running with no way to reach it once
// its window is closed. So the app asks the bus first, and keeps no tray where
// nothing answers that it owns the name.

import { execFile } from 'node:child_process';

/** The bus name a tray host owns. */
export const STATUS_NOTIFIER_WATCHER = 'org.kde.StatusNotifierWatcher';

/** How long the bus is given to answer, in milliseconds. */
export const ASK_TIMEOUT_MS = 1000;

/** The command that asks the session bus whether the watcher's name has an owner. */
export const ASK = {
  command: 'gdbus',
  args: [
    'call',
    '--session',
    '--dest',
    'org.freedesktop.DBus',
    '--object-path',
    '/org/freedesktop/DBus',
    '--method',
    'org.freedesktop.DBus.NameHasOwner',
    STATUS_NOTIFIER_WATCHER,
  ],
} as const;

/** How asking the bus went: what the command printed, or how it failed. */
export type Asked =
  | { kind: 'answered'; stdout: string }
  | { kind: 'timed-out' }
  | { kind: 'missing' }
  | { kind: 'failed' };

/** Whether a tray host is there, and in a word why it is taken to be or not. */
export interface TrayHost {
  present: boolean;
  /** A fixed word for the log: never a path, nor anything the command printed. */
  reason: 'not-linux' | 'owned' | 'not-owned' | 'timed-out' | 'no-gdbus' | 'failed' | 'unreadable';
}

/**
 * What `gdbus` answering `NameHasOwner` comes to: `(true,)` is a host, and
 * everything else — `(false,)`, a timeout, no `gdbus`, an error, or an answer
 * of any other shape — is none.
 */
export function trayHostFrom(asked: Asked): TrayHost {
  switch (asked.kind) {
    case 'timed-out':
      return { present: false, reason: 'timed-out' };
    case 'missing':
      return { present: false, reason: 'no-gdbus' };
    case 'failed':
      return { present: false, reason: 'failed' };
    case 'answered': {
      const answer = asked.stdout.trim();
      if (answer === '(true,)') {
        return { present: true, reason: 'owned' };
      }
      if (answer === '(false,)') {
        return { present: false, reason: 'not-owned' };
      }
      return { present: false, reason: 'unreadable' };
    }
  }
}

/** Runs {@link ASK}, bounded by {@link ASK_TIMEOUT_MS}, and says how it went. */
export function askTheBus(): Promise<Asked> {
  return new Promise((resolve) => {
    execFile(ASK.command, ASK.args, { timeout: ASK_TIMEOUT_MS, encoding: 'utf8' }, (error, stdout) => {
      if (error === null) {
        resolve({ kind: 'answered', stdout });
        return;
      }
      const failure = error as NodeJS.ErrnoException & { killed?: boolean };
      if (failure.code === 'ENOENT') {
        resolve({ kind: 'missing' });
      } else if (failure.killed === true) {
        resolve({ kind: 'timed-out' });
      } else {
        resolve({ kind: 'failed' });
      }
    });
  });
}

/** Whether `platform` shows a tray icon, asking the bus through `ask` on Linux. */
export async function trayHostOn(
  platform: NodeJS.Platform,
  ask: () => Promise<Asked> = askTheBus,
): Promise<TrayHost> {
  if (platform !== 'linux') {
    return { present: true, reason: 'not-linux' };
  }
  try {
    return trayHostFrom(await ask());
  } catch {
    return { present: false, reason: 'failed' };
  }
}

/** The one line the log is told of which way it went. */
export function trayHostLine(host: TrayHost): string {
  return host.present
    ? `a tray icon is shown (${host.reason}); closing the explorer's window hides it`
    : `no tray host on the session bus (${host.reason}); closing the explorer's window quits`;
}
