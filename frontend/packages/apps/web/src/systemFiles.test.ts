import { expect, it } from 'vitest';

import registry from '../../../../../docs/spec/entry-path/system-files.json';
import { ANYWHERE, ANYWHERE_PREFIXES, AT_TOP, isSystemFile, systemCount, systemFilesLine } from './systemFiles';

// EP-17: the page's list is the registry's, the server's being held to the same
// file on its side, so a browser's drop and a scan leave out the same names.
it('declares the names the registry lists', () => {
  expect(ANYWHERE).toEqual(registry.anywhere);
  expect(ANYWHERE_PREFIXES).toEqual(registry.anywhere_prefixes);
  expect(AT_TOP).toEqual(registry.top);
});

it('leaves out a name a system writes anywhere, at any depth', () => {
  for (const name of [
    '.DS_Store',
    'Icon\r',
    'Thumbs.db',
    'ehthumbs.db',
    'desktop.ini',
    '.directory',
    '__MACOSX',
    '._page-001.jpg',
  ]) {
    expect(isSystemFile(name, true), name).toBe(true);
    expect(isSystemFile(name, false), name).toBe(true);
  }
});

it('leaves out a volume’s own folder only directly in a dropped folder', () => {
  for (const name of AT_TOP) {
    expect(isSystemFile(name, true), name).toBe(true);
    expect(isSystemFile(name, false), name).toBe(false);
  }
});

// Byte for byte, case included: a name that only resembles one is the person's.
it('keeps a name that only resembles one', () => {
  for (const name of ['.ds_store', 'Thumbs.DB', 'thumbs.db', '_MACOSX', 'Icon', '.Spotlight-V100x', '_page.jpg']) {
    expect(isSystemFile(name, true), name).toBe(false);
  }
});

it('names a few distinct names, spelling out a carriage return', () => {
  expect(systemFilesLine([])).toBeNull();
  expect(systemFilesLine([{ path: 'a/.DS_Store', folder: false }])).toBe(
    'left out 1 file the system keeps (.DS_Store)',
  );
  expect(
    systemFilesLine([
      { path: 'a/.DS_Store', folder: false },
      { path: 'a/Icon\r', folder: false },
      { path: 'a/Thumbs.db', folder: false },
      { path: 'a/__MACOSX', folder: true },
      { path: 'a/b/.DS_Store', folder: false },
    ]),
  ).toBe('left out 4 files and 1 folder the system keeps (.DS_Store, Icon\\r, Thumbs.db, …)');
});

it('counts the group as "Pack this folder…" names it', () => {
  expect(systemCount(0, 0)).toBeNull();
  expect(systemCount(2, 0)).toBe('2 files the system keeps');
  expect(systemCount(0, 1)).toBe('1 folder the system keeps');
  expect(systemCount(1, 2)).toBe('1 file and 2 folders the system keeps');
});
