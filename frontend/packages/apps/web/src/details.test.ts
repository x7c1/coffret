import { describe, expect, it } from 'vitest';

import type { Freeze, ListedFile, ListedFolder, Listing, Sync } from '@coffret/api';

import {
  DOCUMENT,
  extensionOf,
  fileOnDevice,
  filesOnDevice,
  firstImage,
  folderOnDevice,
  itemsIn,
  kindOf,
  opensInReader,
  selectedHeading,
  summaryOf,
  uploadStopped,
  type Described,
} from './details';
import type { FileRow, FolderRow } from './tree';

function file(path: string, over: Partial<ListedFile> = {}): ListedFile {
  const name = path.slice(path.lastIndexOf('/') + 1);
  const image = /\.(jpe?g|png)$/i.test(name);
  return {
    name,
    path,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: image,
    content_type: image ? 'image/jpeg' : 'application/octet-stream',
    ...over,
  };
}

function folder(path: string, over: Partial<ListedFolder> = {}): ListedFolder {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    mapped: true,
    present_throughout: false,
    files_total: 1,
    files_present: 0,
    size: 0,
    mtime: null,
    ...over,
  };
}

function listing(path: string, folders: ListedFolder[], files: ListedFile[]): Listing {
  return {
    path,
    mapped: true,
    held: true,
    folders,
    files,
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
}

const ROOT = listing('', [], []);

function fileRow(listed: ListedFile): FileRow {
  return { kind: 'file', key: `file:${listed.path}`, path: listed.path, name: listed.name, depth: 0, file: listed, in: ROOT };
}

function folderRow(listed: ListedFolder): FolderRow {
  return {
    kind: 'folder',
    key: `folder:${listed.path}`,
    path: listed.path,
    name: listed.name,
    depth: 0,
    folder: listed,
    in: ROOT,
    expanded: false,
    expandable: true,
    prospective: false,
    stranded: false,
  };
}

describe('kindOf', () => {
  it.each([
    ['cover.jpg', 'JPEG image'],
    ['cover.jpeg', 'JPEG image'],
    ['COVER.JPG', 'JPEG image'],
    ['page-001.png', 'PNG image'],
    ['manual.pdf', 'PDF document'],
    ['notes.txt', 'Plain text'],
    ['clip.mov', 'QuickTime movie'],
    ['archive.xyz', DOCUMENT],
    ['README', DOCUMENT],
    ['.gitignore', DOCUMENT],
  ])('calls %s a %s', (name, kind) => {
    expect(kindOf(name)).toBe(kind);
  });

  it('reads no extension off a name that is all extension or ends in a dot', () => {
    expect(extensionOf('.gitignore')).toBeNull();
    expect(extensionOf('draft.')).toBeNull();
    expect(extensionOf('a.tar.GZ')).toBe('gz');
  });
});

describe('folderOnDevice', () => {
  it.each([
    ['none here', 12_480, 0, '0 of 12,480 files'],
    ['some here', 12_480, 3_210, '3,210 of 12,480 files'],
    ['all here', 12_480, 12_480, 'All 12,480 files'],
    ['its one file here', 1, 1, '1 of 1 file'],
    ['its one file not here', 1, 0, '0 of 1 file'],
    ['no files at all', 0, 0, 'No files'],
  ])('says how many of the files are here, and how many there are: %s', (_, total, present, said) => {
    const counts = { files_total: total, files_present: present, present_throughout: total > 0 && present === total };
    expect(folderOnDevice(folder('book', counts))).toBe(said);
  });
});

describe('filesOnDevice', () => {
  // The Library's "On this device" is said the way a folder's is.
  it.each([
    [12_480, 0, '0 of 12,480 files'],
    [12_480, 3_210, '3,210 of 12,480 files'],
    [12_480, 12_480, 'All 12,480 files'],
    [1, 1, '1 of 1 file'],
    [1, 0, '0 of 1 file'],
    [0, 0, 'No files'],
  ])('says %i files with %i here as %s', (total, present, said) => {
    expect(filesOnDevice(present, total)).toBe(said);
  });
});

describe('fileOnDevice', () => {
  it.each([
    ['present', false, 'Yes'],
    ['remote', false, 'No'],
    ['fetching', false, 'Fetching…'],
    ['failed', false, 'No'],
    ['declined', false, 'No'],
    ['added', false, 'Yes · uploading'],
    ['reserved', false, 'Yes · uploading'],
    ['added', true, 'Yes · upload failed'],
    ['reserved', true, 'Yes · upload failed'],
  ] as const)('says a %s row (upload stopped: %s) reads %s', (state, stopped, said) => {
    expect(fileOnDevice({ state, message: null }, stopped)).toBe(said);
  });
});

describe('uploadStopped', () => {
  const stoppedSync = { status: 'stopped' } as Sync;
  const syncing = { status: 'syncing' } as Sync;
  const stoppedFreeze = (folder: string) => ({ status: 'stopped', folder }) as Freeze;

  it('reads an added file against the sync on record', () => {
    const added = file('albums/new.jpg', { state: 'added', container: null });
    expect(uploadStopped(added, stoppedSync, null)).toBe(true);
    expect(uploadStopped(added, syncing, null)).toBe(false);
    expect(uploadStopped(added, null, null)).toBe(false);
  });

  it('reads a file waiting to be packed against the freeze of the folder it was dropped into', () => {
    const waiting = file('books/vol-1/001.jpg', { state: 'added', container: null, reserved: true });
    expect(uploadStopped(waiting, stoppedSync, stoppedFreeze('books/vol-1'))).toBe(true);
    expect(uploadStopped(waiting, stoppedSync, stoppedFreeze('books/vol-2'))).toBe(false);
    expect(uploadStopped(waiting, stoppedSync, null)).toBe(false);
  });

  it('says nothing stopped of a file the Library holds', () => {
    expect(uploadStopped(file('albums/old.jpg', { state: 'present' }), stoppedSync, null)).toBe(false);
  });
});

describe('opensInReader', () => {
  it('offers Open on an image', () => {
    expect(opensInReader(fileRow(file('albums/cover.jpg')), null)).toBe(true);
  });

  it('offers no Open on a file the reader cannot show', () => {
    expect(opensInReader(fileRow(file('albums/manual.pdf')), null)).toBe(false);
  });

  it('offers Open on a folder holding images, and starts at the first of them', () => {
    const book = listing('book', [folder('book/extras')], [file('book/notes.txt'), file('book/001.jpg'), file('book/002.jpg')]);
    expect(opensInReader(folderRow(folder('book')), book)).toBe(true);
    expect(firstImage(book)?.path).toBe('book/001.jpg');
  });

  it('offers no Open on a folder holding no images, nor on one whose listing is not on hand', () => {
    const papers = listing('papers', [folder('papers/2025')], [file('papers/manual.pdf')]);
    expect(opensInReader(folderRow(folder('papers')), papers)).toBe(false);
    expect(firstImage(papers)).toBeNull();
    expect(opensInReader(folderRow(folder('book')), null)).toBe(false);
  });

  it('counts the items standing directly in a folder', () => {
    const papers = listing('papers', [folder('papers/2025')], [file('papers/manual.pdf')]);
    expect(itemsIn(papers)).toBe(2);
  });
});

describe('summaryOf', () => {
  it('totals a mixed selection of folders and files', () => {
    const rows: Described[] = [
      folderRow(folder('albums', { size: 40_000 })),
      fileRow(file('cover.jpg', { size: 1_500 })),
      folderRow(folder('books', { size: 3_000_000 })),
      fileRow(file('manual.pdf', { size: 2_000_000 })),
      fileRow(file('notes.txt', { size: 500 })),
    ];
    expect(summaryOf(rows)).toEqual({ count: 5, folders: 2, files: 3, bytes: 5_042_000 });
    expect(selectedHeading(5)).toBe('5 items selected');
  });

  it('counts a row below a selected folder once, in the folder', () => {
    const rows: Described[] = [
      folderRow(folder('books', { size: 3_000_000 })),
      folderRow(folder('books/vol-1', { size: 1_000_000 })),
      fileRow(file('books/vol-1/001.jpg', { size: 2_000 })),
      fileRow(file('bookshelf.txt', { size: 7 })),
    ];
    expect(summaryOf(rows)).toEqual({ count: 4, folders: 2, files: 2, bytes: 3_000_007 });
  });

  it('totals nothing selected as nothing', () => {
    expect(summaryOf([])).toEqual({ count: 0, folders: 0, files: 0, bytes: 0 });
  });
});
