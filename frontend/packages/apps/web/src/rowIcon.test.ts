import { describe, expect, it } from 'vitest';

import type { ListedFile, ListedFolder, Listing } from '@coffret/api';

import { rowIconOf } from './rowIcon';
import { treeRows } from './tree';

const folder: ListedFolder = {
  name: 'albums',
  path: 'albums',
  mapped: true,
  present_throughout: false,
  files_total: 1,
  files_present: 0,
  size: 0,
  mtime: null,
};

function file(name: string, openable: boolean, content_type: string): ListedFile {
  return {
    name,
    path: name,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable,
    content_type,
  };
}

/** Each row of a root holding `folders` and `files`, by name, with the icon it draws. */
function icons(folders: ListedFolder[], files: ListedFile[]): Record<string, string> {
  const root: Listing = {
    path: '',
    mapped: true,
    held: true,
    folders,
    files,
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
  const rows = treeRows({
    listing: (path) => (path === '' ? { status: 'ready', value: root } : undefined),
    expanded: new Set(),
    prospective: [],
    stranded: [],
  });
  return Object.fromEntries(rows.flatMap((row) => (row.kind === 'note' ? [] : [[row.name, rowIconOf(row)]])));
}

describe('rowIconOf', () => {
  it('draws a folder for a folder, a picture for a file the reader can show, and a sheet for any other file', () => {
    const files = [file('cover.png', true, 'image/png'), file('notes.pdf', false, 'application/pdf')];
    expect(icons([folder], files)).toEqual({ albums: 'folder', 'cover.png': 'image', 'notes.pdf': 'file' });
  });

  // An image the reader cannot show is not drawn as one: the picture says the
  // reader opens it, which is what `openable` decides, not the file's type.
  it('goes by whether the reader can show the file, not by its type', () => {
    expect(icons([], [file('scan.tiff', false, 'image/tiff')])).toEqual({ 'scan.tiff': 'file' });
  });
});
