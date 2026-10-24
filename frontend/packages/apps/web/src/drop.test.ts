import { expect, it } from 'vitest';

import { filesUnder, type DroppedEntry } from './drop';

/** One file a case dropped, with its content standing in for a real `File`. */
function file(name: string): DroppedEntry {
  return {
    isFile: true,
    isDirectory: false,
    name,
    file: (onFile: (file: File) => void) => onFile(name as unknown as File),
  } as DroppedEntry;
}

/**
 * One folder a case dropped.
 *
 * Its reader answers with the children once and with nothing afterwards, which
 * is what a browser's does: the empty batch is how a folder says it is finished.
 */
function folder(name: string, children: DroppedEntry[]): DroppedEntry {
  return {
    isFile: false,
    isDirectory: true,
    name,
    createReader: () => {
      let left = children;
      return {
        readEntries: (onEntries: (entries: DroppedEntry[]) => void) => {
          const batch = left;
          left = [];
          onEntries(batch);
        },
      };
    },
  } as DroppedEntry;
}

/** A folder whose reader answers one child at a time, as a large one does. */
function inBatches(name: string, children: DroppedEntry[]): DroppedEntry {
  return {
    isFile: false,
    isDirectory: true,
    name,
    createReader: () => {
      let left = [...children];
      return {
        readEntries: (onEntries: (entries: DroppedEntry[]) => void) => {
          const batch = left.slice(0, 1);
          left = left.slice(1);
          onEntries(batch);
        },
      };
    },
  } as DroppedEntry;
}

it('names a file dropped on its own by itself', async () => {
  const { files: added } = await filesUnder([file('spring.jpg')]);
  expect(added.map((one) => one.path)).toEqual(['spring.jpg']);
});

// A folder drop is the same request as a file drop: what the parts carry is the
// path relative to the folder they were dropped on, and the separators in it are
// the folders the server makes.
it('names a file inside a dropped folder by its path under it', async () => {
  const { files: added } = await filesUnder([
    folder('holiday', [
      file('cover.png'),
      folder('day1', [file('one.jpg'), file('two.jpg')]),
    ]),
  ]);

  expect(added.map((one) => one.path)).toEqual([
    'holiday/cover.png',
    'holiday/day1/one.jpg',
    'holiday/day1/two.jpg',
  ]);
});

// A browser answers a large folder a batch at a time and says it is finished by
// answering with none. A walk that read once would silently add the first batch
// and none of the rest, which is a drop that quietly loses files.
it('reads a folder until it says it has no more children', async () => {
  const { files: added } = await filesUnder([inBatches('many', [file('a.jpg'), file('b.jpg'), file('c.jpg')])]);

  expect(added.map((one) => one.path)).toEqual(['many/a.jpg', 'many/b.jpg', 'many/c.jpg']);
});

// A Library has no folders to add — only Entry Paths whose separators imply
// them — so a folder with nothing in it contributes nothing to add.
it('adds nothing for a folder with nothing in it', async () => {
  await expect(filesUnder([folder('empty', [])])).resolves.toEqual({ files: [], systemFiles: [] });
});

// Something dragged that is not a filesystem item at all: a selection of text, an
// image dragged out of another page. There is nothing to add and nothing to
// refuse.
it('passes over an item that is neither a file nor a folder', async () => {
  const neither = { isFile: false, isDirectory: false, name: 'selection' } as DroppedEntry;

  await expect(filesUnder([neither, file('kept.jpg')])).resolves.toMatchObject({
    files: [{ path: 'kept.jpg' }],
    systemFiles: [],
  });
});

// EP-17: what an operating system keeps beside the person's files is left out
// wherever it stands — the dropped item itself included — and handed back apart;
// nothing under a folder of one is walked. A volume's own folder is left out
// directly in a dropped folder and taken deeper down, and a look-alike is the
// person's own.
it('leaves out what an operating system keeps and hands it back apart', async () => {
  const walked = await filesUnder([
    folder('book', [
      file('page-001.jpg'),
      file('.DS_Store'),
      file('._page-001.jpg'),
      file('Thumbs.db'),
      file('Thumbs.DB'),
      folder('__MACOSX', [file('._page-001.jpg')]),
      folder('.Spotlight-V100', [file('index')]),
      folder('inner', [folder('.Spotlight-V100', [file('kept.txt')]), file('desktop.ini')]),
    ]),
    file('.DS_Store'),
  ]);

  expect(walked.files.map((one) => one.path)).toEqual([
    'book/page-001.jpg',
    'book/Thumbs.DB',
    'book/inner/.Spotlight-V100/kept.txt',
  ]);
  expect(walked.systemFiles).toEqual([
    { path: 'book/.DS_Store', folder: false },
    { path: 'book/._page-001.jpg', folder: false },
    { path: 'book/Thumbs.db', folder: false },
    { path: 'book/__MACOSX', folder: true },
    { path: 'book/.Spotlight-V100', folder: true },
    { path: 'book/inner/desktop.ini', folder: false },
    { path: '.DS_Store', folder: false },
  ]);
});
