// The one thing a page may ask of the main process: to take a drop onto the
// explorer's window by the paths of what was let go of. The explorer's preload
// asks it over `TAKE_DROP`; this is what answers, kept apart from Electron so
// that it is tested without it.

import type { DropAnswer } from './addon.js';

/**
 * The channel the explorer's preload takes a drop over. `preload.cts` writes
 * the same name out, since a sandboxed preload imports nothing of the app's.
 */
export const TAKE_DROP = 'coffret:take-drop';

/**
 * The most items one drop may name: as many as one import may name files, since
 * every item dropped is a file or a folder of at least one. A drop is not held
 * to a size — several volumes, or a folder of photographs years deep, are one
 * gesture all the same — and the walk under each dropped folder is the
 * addon's; this bounds only what a page can hand over in one call, against a
 * call of countless paths.
 */
export const MOST_DROPPED_ITEMS = 65_536;

/** The part of an `ipcMain.handle` event this answers by. */
export interface DropAsking {
  /** The contents of the window that asked. */
  sender: unknown;
  /** The frame that asked, where Electron still has it. */
  senderFrame: { url: string } | null;
}

/**
 * The handler `TAKE_DROP` is answered by.
 *
 * Only the explorer's window is answered, and only from the explorer's own
 * origin: `explorer()` is its contents while it is open and `origin()` the
 * origin it is served from, and a call from any other contents or frame is
 * refused. What the page hands over is refused unless it is a list of between
 * one and {@link MOST_DROPPED_ITEMS} strings; each is a whole path on this
 * device, which `take` hands the addon, and what the addon resolves with —
 * a one-time handle and the files' paths relative to where they were dropped
 * from — is the answer. A refusal rejects the page's call.
 *
 * What is said about a refusal is its kind alone: the paths name the person's
 * own files, and none of them reaches the app's output.
 */
export function takeDropHandler(
  explorer: () => unknown,
  origin: () => string | null,
  take: (paths: string[]) => Promise<DropAnswer>,
  warn: (message: string) => void,
): (event: DropAsking, paths: unknown) => Promise<DropAnswer> {
  return async (event, paths) => {
    const window = explorer();
    if (window === null || window === undefined || event.sender !== window) {
      warn('a drop was refused: it did not come from the explorer window');
      throw new Error('a drop is taken from the explorer window alone');
    }
    if (!fromOrigin(event.senderFrame, origin())) {
      warn('a drop was refused: it did not come from the explorer page');
      throw new Error('a drop is taken from the explorer page alone');
    }
    if (!isPathList(paths)) {
      warn('a drop was refused: what was handed over is not a list of paths');
      throw new Error('a drop is a list of the paths of what was dropped');
    }
    return take(paths);
  };
}

function fromOrigin(frame: { url: string } | null, origin: string | null): boolean {
  if (frame === null || origin === null) {
    return false;
  }
  try {
    return new URL(frame.url).origin === origin;
  } catch {
    return false;
  }
}

function isPathList(paths: unknown): paths is string[] {
  return (
    Array.isArray(paths) &&
    paths.length > 0 &&
    paths.length <= MOST_DROPPED_ITEMS &&
    paths.every((path) => typeof path === 'string')
  );
}
