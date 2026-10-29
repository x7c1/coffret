import { describe, expect, it } from 'vitest';

import type { ListedFile, ListedFolder, Listing } from '@coffret/api';

import { actionsFor, deleteTargetOf, type ActionContext } from './actions';
import type { Described } from './details';
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
    content_type: image ? 'image/jpeg' : 'application/pdf',
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

function listing(path: string, files: ListedFile[], over: Partial<Listing> = {}): Listing {
  return {
    path,
    mapped: true,
    held: true,
    folders: [],
    files,
    folders_on_disk: [],
    empty_mapped_prefixes: [],
    ...over,
  };
}

const BOOKS = listing('books', []);

function fileRow(listed: ListedFile): FileRow {
  return { kind: 'file', key: `file:${listed.path}`, path: listed.path, name: listed.name, depth: 1, file: listed, in: BOOKS };
}

function folderRow(listed: ListedFolder, over: Partial<FolderRow> = {}): FolderRow {
  return {
    kind: 'folder',
    key: `folder:${listed.path}`,
    path: listed.path,
    name: listed.name,
    depth: 1,
    folder: listed,
    in: BOOKS,
    expanded: false,
    expandable: true,
    prospective: false,
    stranded: false,
    ...over,
  };
}

/** The folders' own listings: `books/vol-1` holds pages, `books/notes` a PDF alone. */
const LISTINGS: Record<string, Listing> = {
  'books/vol-1': listing('books/vol-1', [file('books/vol-1/p1.jpg')]),
  'books/notes': listing('books/notes', [file('books/notes/a.pdf')]),
};

const CONTEXT: ActionContext = {
  listingOf: (path) => LISTINGS[path] ?? null,
  prepared: ['books/new'],
};

/** The labels the menu shows for `rows`, a separator drawn as `—`. */
function labels(rows: Described[]): string[] {
  return actionsFor(rows, CONTEXT).flatMap((item) => (item.separated ? ['—', item.label] : [item.label]));
}

describe('what the actions menu offers', () => {
  it('offers an image Open and Delete…', () => {
    expect(labels([fileRow(file('books/cover.jpg'))])).toEqual(['Open', '—', 'Delete…']);
  });

  it('offers a non-image file Delete… alone, and a file only added here nothing', () => {
    expect(labels([fileRow(file('books/manual.pdf'))])).toEqual(['Delete…']);
    expect(labels([fileRow(file('books/new.jpg', { state: 'added', container: null }))])).toEqual(['Open']);
    expect(labels([fileRow(file('books/new.pdf', { state: 'added', container: null }))])).toEqual([]);
  });

  it('offers a folder of images Open, New folder…, Pack this folder… and Delete…', () => {
    expect(labels([folderRow(folder('books/vol-1'))])).toEqual([
      'Open',
      'New folder…',
      'Pack this folder…',
      '—',
      'Delete…',
    ]);
    // Without images directly in it — or with its listing not on hand yet —
    // there is nothing for the reader to open on.
    expect(labels([folderRow(folder('books/notes'))])).toEqual(['New folder…', 'Pack this folder…', '—', 'Delete…']);
    expect(labels([folderRow(folder('books/unasked'))])).toEqual([
      'New folder…',
      'Pack this folder…',
      '—',
      'Delete…',
    ]);
  });

  // EP-9: no folder of this device under its top-level folder, so nothing to
  // put a new folder's files in and nothing for a freeze to read.
  it('offers a folder under an unmapped top-level folder only Delete…', () => {
    expect(labels([folderRow(folder('books/far', { mapped: false }))])).toEqual(['Delete…']);
  });

  it('offers a prepared folder Forget this folder instead of Delete…, and an empty mapped one neither', () => {
    const prepared = folderRow(folder('books/new', { files_total: 0 }), { prospective: true, expandable: false });
    expect(labels([prepared])).toEqual(['New folder…', '—', 'Forget this folder']);
    expect(actionsFor([prepared], CONTEXT).some((item) => item.destructive)).toBe(false);

    const mappedPrefix = folderRow(folder('comics', { files_total: 0 }), { prospective: true, expandable: false });
    expect(labels([mappedPrefix])).toEqual(['New folder…']);
  });

  it('draws Delete… as destructive', () => {
    const items = actionsFor([fileRow(file('books/cover.jpg'))], CONTEXT);
    expect(items.map((item) => item.destructive)).toEqual([false, true]);
  });

  it('offers several rows one deletion of all of them, named by count', () => {
    const rows = [folderRow(folder('books/vol-1')), folderRow(folder('books/vol-2')), fileRow(file('books/cover.jpg'))];
    const items = actionsFor(rows, CONTEXT);
    expect(items.map((item) => item.label)).toEqual(['Delete 3 items…']);
    expect(items[0].action).toEqual({
      kind: 'delete',
      target: { folders: ['books/vol-1', 'books/vol-2'], paths: ['books/cover.jpg'] },
    });
  });

  // Rows the Library does not hold have nothing to take out of it: left out of
  // the deletion and the count, and a selection of only such rows offers
  // nothing.
  it('leaves out of a selection’s deletion what the Library does not hold', () => {
    const added = fileRow(file('books/new.jpg', { state: 'added', container: null }));
    const prepared = folderRow(folder('books/new'), { prospective: true });
    const stranded = folderRow(folder('books/packing'), { stranded: true });
    expect(deleteTargetOf([added, prepared, stranded, fileRow(file('books/cover.jpg'))])).toEqual({
      folders: [],
      paths: ['books/cover.jpg'],
    });
    expect(labels([added, prepared, stranded, fileRow(file('books/cover.jpg'))])).toEqual(['Delete 1 item…']);
    expect(labels([added, prepared])).toEqual([]);
  });
});
