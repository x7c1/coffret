// Where the app finds what it carries beside its own code: the Node-API addon
// it runs the server in, and the icons under the package's `assets/`.
//
// Run from a checkout, both are where the checkout keeps them: the addon where
// `make napi-addon` leaves it, and the icons beside `dist/`. Packaged, both are
// files of their own under the app's resources directory
// (`process.resourcesPath`), outside its asar archive, since Node loads an
// addon from a file on disk and the packaging config ships them there
// (`electron-builder.config.js`).

import { join, resolve } from 'node:path';

/** What the app loads from beside its code. */
export interface Resources {
  /** The Node-API addon, coffret-napi built with the explorer compiled in. */
  addon: string;
  /** The directory of the app's icons. */
  assets: string;
  /** What to do where the addon is missing, said after its path. */
  missingAddon: string;
}

/** The addon's file name, in the checkout's target directory and in a package. */
export const ADDON_FILE = 'coffret.node';

/**
 * Where `make napi-addon` leaves the addon, from `dist/` of this package in
 * the checkout (`frontend/packages/apps/desktop/dist/`).
 */
const ADDON_FROM_BUILT = ['..', '..', '..', '..', '..', 'backend', 'target', 'release', ADDON_FILE];

/**
 * The resources of an app run from a package, under `resourcesPath`, or from
 * the checkout whose `dist/` is `built`.
 */
export function resourcesFor(packaged: boolean, resourcesPath: string, built: string): Resources {
  if (packaged) {
    return {
      addon: join(resourcesPath, ADDON_FILE),
      assets: join(resourcesPath, 'assets'),
      missingAddon: 'install Coffret again to restore it.',
    };
  }
  return {
    addon: resolve(built, ...ADDON_FROM_BUILT),
    assets: resolve(built, '..', 'assets'),
    missingAddon: 'run make napi-addon.',
  };
}
