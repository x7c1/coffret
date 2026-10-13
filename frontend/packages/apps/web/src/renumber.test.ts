import { describe, expect, it } from 'vitest';

import type { Added } from '@coffret/api';

import { NO_NUMBERING, renumber, unchangedLine, type Numbering } from './renumber';

/** One dropped file at `path`, relative to the folder dropped onto. */
function added(path: string): Added {
  return { path, file: { size: 10 } as File };
}

const PAGES: Numbering = { pages: true, volumes: false };
const VOLUMES: Numbering = { pages: false, volumes: true };
const BOTH: Numbering = { pages: true, volumes: true };

/** The paths the drop is sent under. */
function paths(files: Added[], numbering: Numbering): string[] {
  return renumber(files, numbering).files.map((file) => file.path);
}

/** One file for every number from `first` to `last`, named by `name`. */
function range(first: number, last: number, name: (at: number) => string): Added[] {
  return Array.from({ length: last - first + 1 }, (_, at) => added(name(first + at)));
}

describe('renumber', () => {
  it('numbers the pages of a scanned book by the number in their names', () => {
    const files = range(1, 210, (at) => `king01/king01-#${String(at).padStart(3, '0')}.jpg`);
    const sent = paths(files, PAGES);
    expect(sent[0]).toBe('king01/001.jpg');
    expect(sent[209]).toBe('king01/210.jpg');
    expect(new Set(sent).size).toBe(210);
  });

  it('numbers volume folders, keeping the pages under them', () => {
    const files = range(1, 12, (at) => `series/第${at}巻/p.jpg`);
    const result = renumber(files, VOLUMES);
    expect(result.files.map((file) => file.path)).toEqual(
      range(1, 12, (at) => `series/${String(at).padStart(3, '0')}/p.jpg`).map((file) => file.path),
    );
    expect(result.groups).toHaveLength(1);
    expect(result.groups[0]).toMatchObject({ kind: 'volumes', folder: 'series', leftWhole: null });
    expect(result.groups[0].renamed[0]).toEqual({ from: '第1巻', to: '001' });
  });

  // The value is kept: a later volume or a page found later lines up with what
  // was numbered before, which reassigning 1..N would not do.
  it('keeps a gap in the sequence a gap', () => {
    expect(paths([added('b/p1.jpg'), added('b/p2.jpg'), added('b/p5.jpg')], PAGES)).toEqual([
      'b/001.jpg',
      'b/002.jpg',
      'b/005.jpg',
    ]);
  });

  it('lowercases the extension of a page', () => {
    expect(paths([added('b/IMG_7.JPG'), added('b/IMG_8.Jpeg')], PAGES)).toEqual([
      'b/007.jpg',
      'b/008.jpeg',
    ]);
  });

  it('widens the whole group to the largest number', () => {
    expect(paths([added('b/p-7.jpg'), added('b/p-1000.jpg')], PAGES)).toEqual([
      'b/0007.jpg',
      'b/1000.jpg',
    ]);
  });

  it('leaves a name without a number alone and says so', () => {
    const result = renumber([added('b/p1.jpg'), added('b/p2.jpg'), added('b/cover.png')], PAGES);
    expect(result.files.map((file) => file.path)).toEqual(['b/001.jpg', 'b/002.jpg', 'b/cover.png']);
    expect(result.groups).toEqual([
      {
        kind: 'pages',
        folder: 'b',
        renamed: [
          { from: 'p1.jpg', to: '001.jpg' },
          { from: 'p2.jpg', to: '002.jpg' },
        ],
        unchanged: [{ from: 'cover.png', to: 'cover.png' }],
        leftWhole: null,
      },
    ]);
  });

  it('leaves a group whole where two names carry the same number', () => {
    const files = [added('b/p01.jpg'), added('b/p1.jpg'), added('b/p2.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: 'same_number' });
    expect(result.groups[0].unchanged).toHaveLength(3);
  });

  // A file already called 001.jpg would be replaced by the page renamed onto
  // it, so the group is not renamed at all.
  it('leaves a group whole where a new name is a sibling that is not renamed', () => {
    const files = [
      added('b/king01-#001.jpg'),
      added('b/king01-#002.jpg'),
      added('b/king01-#003.jpg'),
      added('b/001.jpg'),
    ];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0].leftWhole).toBe('taken');
  });

  it('renames nothing where two frames are equally common', () => {
    const files = [added('b/a1.jpg'), added('b/a2.jpg'), added('b/b1.jpg'), added('b/b2.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0].leftWhole).toBe('tie');
  });

  it('renames only the most common frame', () => {
    const files = [
      added('b/page-1.jpg'),
      added('b/page-2.jpg'),
      added('b/page-3.jpg'),
      added('b/extra-9.jpg'),
    ];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual([
      'b/001.jpg',
      'b/002.jpg',
      'b/003.jpg',
      'b/extra-9.jpg',
    ]);
    expect(result.groups[0].unchanged).toEqual([{ from: 'extra-9.jpg', to: 'extra-9.jpg' }]);
  });

  it('never renames the dropped folder itself', () => {
    expect(paths([added('vol1/p1.jpg'), added('vol2/p1.jpg')], BOTH)).toEqual([
      'vol1/001.jpg',
      'vol2/001.jpg',
    ]);
  });

  // A folder named like a page, or a page named like a folder, is not part of
  // the other's sequence.
  it('judges volumes among sibling folders and pages among sibling files', () => {
    const files = [
      added('book/vol1/king-#1.jpg'),
      added('book/vol1/king-#2.jpg'),
      added('book/vol2/king-#1.jpg'),
      added('book/vol3/king-#1.jpg'),
      added('book/vol9.jpg'),
      added('book/cover.png'),
    ];
    expect(paths(files, VOLUMES)).toEqual([
      'book/001/king-#1.jpg',
      'book/001/king-#2.jpg',
      'book/002/king-#1.jpg',
      'book/003/king-#1.jpg',
      'book/vol9.jpg',
      'book/cover.png',
    ]);
    const both = renumber(files, BOTH);
    expect(both.files.map((file) => file.path)).toEqual([
      'book/001/001.jpg',
      'book/001/002.jpg',
      'book/002/001.jpg',
      'book/003/001.jpg',
      'book/009.jpg',
      'book/cover.png',
    ]);
    // The preview names a page under its renamed volume.
    const firstVolume = both.groups.find((group) => group.folder === 'book/vol1');
    expect(firstVolume?.renamed[0]).toEqual({ from: 'vol1/king-#1.jpg', to: '001/001.jpg' });
  });

  it('says where a kept name lands when its volume is renamed', () => {
    const result = renumber(
      [added('book/vol1/p1.jpg'), added('book/vol1/cover.png'), added('book/vol2/p1.jpg')],
      BOTH,
    );
    const firstVolume = result.groups.find(
      (group) => group.kind === 'pages' && group.folder === 'book/vol1',
    );
    expect(firstVolume?.unchanged).toEqual([{ from: 'vol1/cover.png', to: '001/cover.png' }]);
    expect(unchangedLine(firstVolume!.unchanged[0])).toBe(
      'vol1/cover.png → 001/cover.png (name unchanged)',
    );
    expect(unchangedLine({ from: 'cover.png', to: 'cover.png' })).toBe('cover.png (unchanged)');
  });

  it('returns every path unchanged with both options off', () => {
    const files = [added('book/vol1/p1.jpg'), added('book/p2.JPG'), added('loose3.jpg')];
    const result = renumber(files, NO_NUMBERING);
    expect(result.files).toEqual(files);
    expect(result.groups).toEqual([]);
  });

  it('leaves files dropped loose alone', () => {
    expect(paths([added('p1.jpg'), added('p2.jpg'), added('b/p1.jpg')], PAGES)).toEqual([
      'p1.jpg',
      'p2.jpg',
      'b/001.jpg',
    ]);
  });
});
