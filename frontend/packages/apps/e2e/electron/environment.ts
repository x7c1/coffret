// What the run this suite belongs to has already built, as the script that
// built it says so.
//
// Nothing here has a default, for the reason the browser journeys' environment
// has none: the suite is one half of `make e2e-electron` — the other half put a
// Library on MinIO and built the app — so a variable that is missing means the
// suite was started on its own.

import path from 'node:path';

/** What `scripts/e2e-electron.sh` hands over. */
export interface Environment {
  /** The Electron binary the desktop package runs on. */
  electronBinary: string;
  /** The desktop package, which is the app Electron is started on. */
  appDir: string;
  /** Extra switches for Electron, one per line; empty where there are none. */
  electronSwitches: string[];
  /** The throwaway state directory the app serves its Libraries from. */
  stateDir: string;
  /** Where the app's log goes, which the suite reads the idle lock out of. */
  logDir: string;
  /** The throwaway directory the app keeps its browser data in. */
  userDataDir: string;
  /** What the device calls the Library the suite opens. */
  library: string;
  /** The Passphrase that opens it; a fixed test string. */
  passphrase: string;
  /** The Library's mapped top-level folder, and the folder under it the suite walks. */
  mapped: string;
  album: string;
  /** The first photograph in that folder, which the explorer lists. */
  firstPhoto: string;
  /** A JPEG outside the Library, which the suite drops onto the folder. */
  dropFile: string;
  /** The idle interval the app is started with, in minutes. */
  idleMinutes: number;
  /** Where Playwright's own traces and failure shots go. */
  artifacts: string;
  /** Where the checkpoint pictures are written. */
  screenshots: string;
}

/** What the script said, or a refusal naming what it did not say. */
export function fromEnvironment(): Environment {
  const switches = process.env.COFFRET_E2E_ELECTRON_SWITCHES ?? '';
  return {
    electronBinary: required('COFFRET_E2E_ELECTRON_BIN'),
    appDir: required('COFFRET_E2E_APP_DIR'),
    electronSwitches: switches.split('\n').filter((line) => line !== ''),
    stateDir: required('COFFRET_E2E_STATE_DIR'),
    logDir: required('COFFRET_E2E_LOG_DIR'),
    userDataDir: required('COFFRET_E2E_USER_DATA_DIR'),
    library: required('COFFRET_E2E_LIBRARY'),
    passphrase: required('COFFRET_E2E_PASSPHRASE'),
    mapped: required('COFFRET_E2E_MAPPED'),
    album: required('COFFRET_E2E_ALBUM'),
    firstPhoto: required('COFFRET_E2E_FIRST_PHOTO'),
    dropFile: required('COFFRET_E2E_DROP_FILE'),
    idleMinutes: number('COFFRET_E2E_IDLE_MINUTES'),
    artifacts: required('COFFRET_E2E_ARTIFACTS'),
    screenshots: required('COFFRET_E2E_SCREENSHOTS'),
  };
}

/** The last component of an Entry Path, which is what a folder is called. */
export function leaf(entry: string): string {
  return path.posix.basename(entry);
}

function required(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === '') {
    throw new Error(
      `${name} is not set. This journey runs the desktop app against a Library on MinIO that ` +
        'scripts/e2e-electron.sh builds; start it with `make e2e-electron` rather than on its own.',
    );
  }
  return value;
}

function number(name: string): number {
  const value = Number(required(name));
  if (!Number.isInteger(value) || value < 1) {
    throw new Error(`${name} is not a whole number of at least one: ${process.env[name]}`);
  }
  return value;
}
