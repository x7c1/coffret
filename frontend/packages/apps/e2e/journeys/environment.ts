// What the run this suite belongs to has already built, as the script that
// built it says so.
//
// Nothing here has a default. The suite is one half of `make e2e-it` — the
// other half put a Library on MinIO, joined a second device to it and generated
// the files the journeys walk through — so a variable that is missing means the
// suite was started on its own, and a default would send it at whatever
// happened to be listening instead of saying so.

import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** This directory, which is where every path below is measured from. */
const HERE = path.dirname(fileURLToPath(import.meta.url));

/** What `scripts/e2e-it.sh` hands over. */
export interface Environment {
  /** The `coffret-server` binary the script built. */
  serverBinary: string;
  /** The joined device's state directory, which is where its Library is. */
  stateDir: string;
  /** What that device calls the Library, which is what the server serves. */
  library: string;
  /** Where the server's own output goes, so a failed run has it to read. */
  logDir: string;
  /** The Passphrase that opens the Library; a fixed test string. */
  passphrase: string;
  /** The loopback port the server is started on, and restarted on. */
  serverPort: number;
  /** The loopback port the built explorer is previewed at. */
  webPort: number;
  /** Where the explorer is, as a page addresses it. */
  webUrl: string;
  /** The web package, which is what `vite preview` is run from. */
  webPackage: string;
  /** Where the checkpoint pictures a person reviews are written. */
  screenshots: string;
  /** Where Playwright's own traces and failure shots go. */
  artifacts: string;
  /** The album folder in the Library, as an Entry Path. */
  album: string;
  /** The book folder in the Library, as an Entry Path. */
  book: string;
  /** How many photos the album holds, and how many pages the book does. */
  photos: number;
  pages: number;
  /** A JPEG outside the Library, for the drop journey to drop. */
  dropFile: string;
  /** The `coffret` binary the script built, which the other device is run as. */
  cliBinary: string;
  /** That device's state directory, which is where its Library is. */
  uploaderStateDir: string;
  /** What that device calls the Library. */
  uploaderLibrary: string;
  /** The folder it maps the Library's mapped prefix at (spec: EP-9). */
  uploaderRoot: string;
  /** A JPEG for the other device to commit while this one is looking. */
  refreshFile: string;
  /** And one for it to commit while this one's server is stopped. */
  restartFile: string;
}

/** What the script said, or a refusal naming what it did not say. */
export function fromEnvironment(): Environment {
  const serverPort = number('COFFRET_E2E_SERVER_PORT');
  const webPort = number('COFFRET_E2E_WEB_PORT');
  return {
    serverBinary: required('COFFRET_E2E_SERVER_BIN'),
    stateDir: required('COFFRET_E2E_STATE_DIR'),
    library: required('COFFRET_E2E_LIBRARY'),
    logDir: required('COFFRET_E2E_LOG_DIR'),
    passphrase: required('COFFRET_E2E_PASSPHRASE'),
    serverPort,
    webPort,
    webUrl: `http://127.0.0.1:${webPort}/`,
    webPackage: path.resolve(HERE, '../../web'),
    screenshots: required('COFFRET_E2E_SCREENSHOTS'),
    artifacts: required('COFFRET_E2E_ARTIFACTS'),
    album: required('COFFRET_E2E_ALBUM'),
    book: required('COFFRET_E2E_BOOK'),
    photos: number('COFFRET_E2E_PHOTOS'),
    pages: number('COFFRET_E2E_PAGES'),
    dropFile: required('COFFRET_E2E_DROP_FILE'),
    cliBinary: required('COFFRET_E2E_CLI_BIN'),
    uploaderStateDir: required('COFFRET_E2E_UPLOADER_STATE_DIR'),
    uploaderLibrary: required('COFFRET_E2E_UPLOADER_LIBRARY'),
    uploaderRoot: required('COFFRET_E2E_UPLOADER_ROOT'),
    refreshFile: required('COFFRET_E2E_REFRESH_FILE'),
    restartFile: required('COFFRET_E2E_RESTART_FILE'),
  };
}

function required(name: string): string {
  const value = process.env[name];
  if (value === undefined || value === '') {
    throw new Error(
      `${name} is not set. These journeys run against a Library on MinIO that ` +
        'scripts/e2e-it.sh builds; start them with `make e2e-it` rather than on their own.',
    );
  }
  return value;
}

function number(name: string): number {
  const value = Number(required(name));
  if (!Number.isInteger(value)) {
    throw new Error(`${name} is ${process.env[name]}, which is not a whole number.`);
  }
  return value;
}
