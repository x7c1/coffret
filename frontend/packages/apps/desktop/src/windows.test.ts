import { describe, expect, it } from 'vitest';

import {
  EXPLORER_PARTITION,
  explorerWindowOptions,
  PARTITIONS,
  PASSPHRASE_PARTITION,
  passphraseWindowOptions,
} from './windows.js';

const PRELOAD = '/app/dist/preload.cjs';

const windows = {
  passphrase: passphraseWindowOptions(),
  explorer: explorerWindowOptions(PRELOAD),
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

describe('appearance', () => {
  it('titles every window Coffret, with no icon of its own, by default', () => {
    for (const options of Object.values(windows)) {
      expect(options.title).toBe('Coffret');
      expect(options).not.toHaveProperty('icon');
    }
  });

  it('carries the title and icon it is given', () => {
    const appearance = { title: 'Coffret (dev)', icon: '/app/assets/icon-dev.png' };
    for (const options of [passphraseWindowOptions(appearance), explorerWindowOptions(PRELOAD, appearance)]) {
      expect(options.title).toBe('Coffret (dev)');
      expect(options.icon).toBe('/app/assets/icon-dev.png');
    }
  });
});
