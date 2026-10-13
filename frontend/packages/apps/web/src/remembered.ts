// One yes-or-no the explorer remembers across drops and reloads, kept free of
// React so it is unit testable.
//
// Browser storage is a convenience here and never something a drop depends
// on: a private window, a browser that blocks site data, or a full quota each
// make reading or writing it throw, and any of those must leave the drop
// working exactly as it would for somebody who never ticked a box. So every
// access is caught, and what cannot be read is the default.

/** Where the value is kept, asked for lazily because asking can throw too. */
export type Storing = () => Pick<Storage, 'getItem' | 'setItem'>;

/** The page's own storage, which is what everything but a test uses. */
const PAGE: Storing = () => window.localStorage;

/** What a value that was never written, or cannot be read, is taken to be. */
const DEFAULT = false;

/** The value remembered under `key`, or `false` where there is none to read. */
export function recall(key: string, storing: Storing = PAGE): boolean {
  try {
    const held = storing().getItem(key);
    return held === null ? DEFAULT : held === 'true';
  } catch (refused) {
    console.warn(`the explorer could not read ${key} from browser storage`, refused);
    return DEFAULT;
  }
}

/**
 * Remembers `value` under `key`, or does nothing where storage refuses it.
 *
 * The box stays ticked on the screen either way; only the next drop forgets.
 */
export function remember(key: string, value: boolean, storing: Storing = PAGE): void {
  try {
    storing().setItem(key, value ? 'true' : 'false');
  } catch (refused) {
    console.warn(`the explorer could not write ${key} to browser storage`, refused);
  }
}
