import { describe, expect, it } from 'vitest';

import { menuTemplate, RELOAD_THE_EXPLORER } from './menu.js';

import {
  EXPLORER_PARTITION,
  explorerWindowOptions,
  PARTITIONS,
  PASSPHRASE_PARTITION,
  passphraseWindowOptions,
} from './windows.js';

const PRELOAD = '/app/dist/preload.cjs';

const windows = {
  passphrase: passphraseWindowOptions('linux'),
  explorer: explorerWindowOptions(PRELOAD, 'linux'),
};

describe.each(Object.entries(windows))('the %s window', (_name, options) => {
  const preferences = options.webPreferences ?? {};

  it('is sandboxed and isolated, without Node', () => {
    expect(preferences.sandbox).toBe(true);
    expect(preferences.contextIsolation).toBe(true);
    expect(preferences.nodeIntegration).toBe(false);
    expect(preferences.nodeIntegrationInWorker).toBe(false);
    expect(preferences.nodeIntegrationInSubFrames).toBe(false);
  });

  it('keeps web security', () => {
    expect(preferences.webSecurity).toBe(true);
    expect(preferences.allowRunningInsecureContent).toBe(false);
    expect(preferences.webviewTag).toBe(false);
  });
});

describe('preloads', () => {
  it('are the explorer window’s alone', () => {
    expect(windows.explorer.webPreferences?.preload).toBe(PRELOAD);
    expect(windows.passphrase.webPreferences?.preload).toBeUndefined();
  });
});

describe('partitions', () => {
  it('give each window one of its own', () => {
    expect(windows.passphrase.webPreferences?.partition).toBe(PASSPHRASE_PARTITION);
    expect(windows.explorer.webPreferences?.partition).toBe(EXPLORER_PARTITION);
    expect(PASSPHRASE_PARTITION).not.toBe(EXPLORER_PARTITION);
    expect(PARTITIONS).toEqual([PASSPHRASE_PARTITION, EXPLORER_PARTITION]);
  });

  it('keep the explorer’s on disk and the Passphrase window’s in memory', () => {
    expect(EXPLORER_PARTITION.startsWith('persist:')).toBe(true);
    expect(PASSPHRASE_PARTITION.startsWith('persist:')).toBe(false);
  });
});

describe('the Passphrase window', () => {
  it('is small and fixed in size', () => {
    expect(windows.passphrase).toMatchObject({ width: 380, height: 260, resizable: false });
  });
});

describe('the windows’ look', () => {
  it('titles every window Coffret, with no icon of its own, by default', () => {
    for (const options of Object.values(windows)) {
      expect(options.title).toBe('Coffret');
      expect(options).not.toHaveProperty('icon');
    }
  });

  it('carries the title and icon it is given', () => {
    const look = { title: 'Coffret (dev)', icon: '/app/assets/icon-dev.png' };
    for (const options of [passphraseWindowOptions('linux', look), explorerWindowOptions(PRELOAD, 'linux', look)]) {
      expect(options.title).toBe('Coffret (dev)');
      expect(options.icon).toBe('/app/assets/icon-dev.png');
    }
  });
});

describe('the menu bar', () => {
  // Hidden on Linux and Windows, where the window would draw it, and shown
  // again on Alt.
  it.each(['linux', 'win32'] as const)('is hidden on %s', (platform) => {
    expect(passphraseWindowOptions(platform).autoHideMenuBar).toBe(true);
    expect(explorerWindowOptions(PRELOAD, platform).autoHideMenuBar).toBe(true);
  });

  // macOS's bar is the system's, and its Edit menu carries Cmd+C, Cmd+V and
  // Cmd+Z to the inputs: nothing of the window's to hide.
  it('is left alone on macOS', () => {
    expect(passphraseWindowOptions('darwin')).not.toHaveProperty('autoHideMenuBar');
    expect(explorerWindowOptions(PRELOAD, 'darwin')).not.toHaveProperty('autoHideMenuBar');
  });

  // Hiding the bar hides no menu: the application's still carries Ctrl+R and
  // Ctrl+Q on Linux.
  it('still has a menu behind it on Linux, with Ctrl+R and Ctrl+Q', () => {
    const items = (template: ReturnType<typeof menuTemplate>): ReturnType<typeof menuTemplate> =>
      template.flatMap((item) => [item, ...(Array.isArray(item.submenu) ? items(item.submenu) : [])]);
    const all = items(menuTemplate('linux', () => undefined));
    expect(all.find((each) => each.label === RELOAD_THE_EXPLORER)?.accelerator).toBe('CmdOrCtrl+R');
    expect(all.find((each) => each.role === 'quit')?.accelerator).toBe('Ctrl+Q');
  });
});
