import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

import { identityFor } from './identity.js';
import { DEV_IDENTIFIER, IDENTIFIER } from './settings.js';

/** The package's assets, which every file named here has to be among. */
function asset(name: string): string {
  return fileURLToPath(new URL(`../assets/${name}`, import.meta.url));
}

describe('the installed app', () => {
  it('on macOS shows the black template tray icon, and no icons of its own', () => {
    expect(identityFor(IDENTIFIER, 'darwin')).toEqual({
      title: 'Coffret',
      tooltip: 'Coffret',
      trayIcon: 'tray.png',
      trayScale: 2,
      trayTemplate: true,
      windowIcon: null,
      dockIcon: null,
      linuxAppId: null,
    });
  });

  it('on Linux shows the light tray icon, keeps the default window icon, and carries its identifier as the app ID', () => {
    expect(identityFor(IDENTIFIER, 'linux')).toEqual({
      title: 'Coffret',
      tooltip: 'Coffret',
      trayIcon: 'tray-light.png',
      trayScale: 1,
      trayTemplate: false,
      windowIcon: null,
      dockIcon: null,
      linuxAppId: IDENTIFIER,
    });
  });
});

describe('the development app', () => {
  it('on macOS is badged in the tray and the Dock, and says so in its titles and tooltip', () => {
    expect(identityFor(DEV_IDENTIFIER, 'darwin')).toEqual({
      title: 'Coffret (dev)',
      tooltip: 'Coffret (dev)',
      trayIcon: 'tray-dev.png',
      trayScale: 2,
      trayTemplate: true,
      windowIcon: null,
      dockIcon: 'dock-dev.png',
      linuxAppId: null,
    });
  });

  it('on Linux is badged in the tray and on its windows, and carries its identifier as the app ID', () => {
    expect(identityFor(DEV_IDENTIFIER, 'linux')).toEqual({
      title: 'Coffret (dev)',
      tooltip: 'Coffret (dev)',
      trayIcon: 'tray-dev-light.png',
      trayScale: 1,
      trayTemplate: false,
      windowIcon: 'icon-dev.png',
      dockIcon: null,
      linuxAppId: DEV_IDENTIFIER,
    });
  });
});

describe('every icon named', () => {
  it('is among the package’s assets, with its @2x file beside each tray icon', () => {
    for (const identifier of [IDENTIFIER, DEV_IDENTIFIER]) {
      for (const platform of ['darwin', 'linux', 'win32'] as const) {
        const identity = identityFor(identifier, platform);
        const tray2x = identity.trayIcon.replace(/\.png$/, '@2x.png');
        for (const name of [identity.trayIcon, tray2x, identity.windowIcon, identity.dockIcon]) {
          if (name !== null) {
            expect(existsSync(asset(name)), name).toBe(true);
          }
        }
      }
    }
  });
});
