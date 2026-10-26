import { describe, expect, it } from 'vitest';

import { addedFile, type Added } from '@coffret/api';

import { NO_NUMBERING, renumber, unchangedLine, type Numbering } from './renumber';
import { inLibraryOrder } from './stranded';

/** One dropped file at `path`, relative to the folder dropped onto. */
function added(path: string): Added {
  return addedFile(path, { size: 10, lastModified: 0 } as File);
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

  // A cover kept as `cover.png` would sort after every numbered page, since
  // digits sort before letters; numbered after the page before it, it stays
  // where it was read.
  it('numbers a name without a number after the page before it, and says so', () => {
    const result = renumber([added('b/p1.jpg'), added('b/p2.jpg'), added('b/Cover.PNG')], PAGES);
    expect(result.files.map((file) => file.path)).toEqual([
      'b/001.jpg',
      'b/002.jpg',
      'b/000_Cover.png',
    ]);
    expect(result.groups).toEqual([
      {
        kind: 'pages',
        folder: 'b',
        renamed: [
          { from: 'Cover.PNG', to: '000_Cover.png' },
          { from: 'p1.jpg', to: '001.jpg' },
          { from: 'p2.jpg', to: '002.jpg' },
        ],
        unchanged: [],
        leftWhole: null,
      },
    ]);
  });

  it('keeps the order of a book with a cover and a spread', () => {
    const files = [
      added('book/cover.jpg'),
      ...range(1, 17, (at) => `book/p${String(at).padStart(4, '0')}.jpg`),
      added('book/p0018+p0019.jpg'),
      added('book/p0020.jpg'),
    ];
    const sent = paths(files, PAGES);
    expect(sent).toEqual([
      'book/000_cover.jpg',
      ...range(1, 17, (at) => `book/${String(at).padStart(3, '0')}.jpg`).map((file) => file.path),
      'book/018+019.jpg',
      'book/020.jpg',
    ]);
    // Sorted as the Library sorts, the new names are in the order the old
    // ones were read in.
    expect([...sent].sort(inLibraryOrder)).toEqual(sent);
  });

  it('numbers a spread joined by a hyphen, or with no text around its numbers', () => {
    expect(paths([added('b/p1.jpg'), added('b/p2-p3.jpg'), added('b/p4.jpg')], PAGES)).toEqual([
      'b/001.jpg',
      'b/002-003.jpg',
      'b/004.jpg',
    ]);
    expect(paths([added('b/0017.jpg'), added('b/0018+0019.jpg'), added('b/0020.jpg')], PAGES)).toEqual([
      'b/017.jpg',
      'b/018+019.jpg',
      'b/020.jpg',
    ]);
  });

  it('numbers a name after a spread by the spread\'s last number', () => {
    expect(
      paths([added('b/p17.jpg'), added('b/p18+p19.jpg'), added('b/p19x.jpg'), added('b/p20.jpg')], PAGES),
    ).toEqual(['b/017.jpg', 'b/018+019.jpg', 'b/019_p19x.jpg', 'b/020.jpg']);
  });

  it('widens the whole group to the largest number of a spread', () => {
    expect(paths([added('b/p1.jpg'), added('b/p2.jpg'), added('b/p999+p1000.jpg')], PAGES)).toEqual([
      'b/0001.jpg',
      'b/0002.jpg',
      'b/0999+1000.jpg',
    ]);
  });

  // By bytes `p10` comes before `p2`; a file manager, and so the person, puts
  // it after, and `P2a` between them whatever its case.
  it('places a numbered-after name by the natural order of the names', () => {
    expect(
      paths([added('b/p10.jpg'), added('b/p2.jpg'), added('b/P2a.jpg'), added('b/p3.jpg')], PAGES),
    ).toEqual(['b/010.jpg', 'b/002.jpg', 'b/002_P2a.jpg', 'b/003.jpg']);
  });

  // Not yet seen in a book, so read as any other name rather than guessed at.
  it('numbers three joined pages after the page before them', () => {
    expect(paths([added('b/p1.jpg'), added('b/p2+p3+p4.jpg'), added('b/p5.jpg')], PAGES)).toEqual([
      'b/001.jpg',
      'b/001_p2+p3+p4.jpg',
      'b/005.jpg',
    ]);
  });

  it('leaves a group whole where two names carry the same number', () => {
    const files = [added('b/p01.jpg'), added('b/p1.jpg'), added('b/p2.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: 'same_number' });
    expect(result.groups[0].unchanged).toHaveLength(3);
  });

  // `p2` beside `p2+p3` is page 2 scanned twice, which numbering would not
  // tell apart.
  it('leaves a group whole where a page carries a number of a spread', () => {
    const files = [added('b/p1.jpg'), added('b/p2.jpg'), added('b/p2+p3.jpg'), added('b/p4.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: 'same_number' });
  });

  // `x.JPG` and `x.jpg` both become `002_x.jpg`, and the second to land would
  // replace the first.
  it('leaves a group whole where two names would be numbered to one', () => {
    const files = [added('b/p1.jpg'), added('b/p2.jpg'), added('b/x.JPG'), added('b/x.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: 'taken' });
  });

  // A file already called 001.jpg is read before the first page, and goes
  // there rather than being replaced by the page renamed onto it.
  it('numbers a file already called by a new name out of its way', () => {
    const files = [
      added('b/king01-#001.jpg'),
      added('b/king01-#002.jpg'),
      added('b/king01-#003.jpg'),
      added('b/001.jpg'),
    ];
    expect(paths(files, PAGES)).toEqual(['b/001.jpg', 'b/002.jpg', 'b/003.jpg', 'b/000_001.jpg']);
  });

  // A page renamed onto a folder beside it could not be put there at all.
  it('leaves a group whole where a new name is a sibling folder', () => {
    const files = [added('b/p1.jpg'), added('b/p2.jpg'), added('b/001.jpg/x.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    const pages = result.groups.find((group) => group.folder === 'b');
    expect(pages?.leftWhole).toBe('taken');
  });

  it('leaves a group whole where a name comes before page 0', () => {
    const files = [added('b/cover.jpg'), added('b/p0.jpg'), added('b/p1.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: 'order' });
  });

  it('numbers from page 0 where nothing comes before it', () => {
    expect(paths([added('b/p0.jpg'), added('b/p1.jpg'), added('b/x.jpg')], PAGES)).toEqual([
      'b/000.jpg',
      'b/001.jpg',
      'b/001_x.jpg',
    ]);
  });

  // `a2` and `a10` both go before page 1, and between themselves would sort
  // `000_a10` first by bytes, which is not the order they are read in.
  it('leaves a group whole where the new names would sort out of order', () => {
    const files = [
      added('b/p1.jpg'),
      added('b/p2.jpg'),
      added('b/p3.jpg'),
      added('b/a2.jpg'),
      added('b/a10.jpg'),
    ];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: 'order' });
    expect(result.groups[0].unchanged).toHaveLength(5);
  });

  it('renames nothing where two frames are equally common', () => {
    const files = [added('b/a1.jpg'), added('b/a2.jpg'), added('b/b1.jpg'), added('b/b2.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0].leftWhole).toBe('tie');
  });

  it('numbers a name in a less common frame after the page before it', () => {
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
      'b/000_extra-9.jpg',
    ]);
    expect(result.groups[0].unchanged).toEqual([]);
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
      'book/008_cover.png',
    ]);
    // The preview names a page under its renamed volume.
    const firstVolume = both.groups.find((group) => group.folder === 'book/vol1');
    expect(firstVolume?.renamed[0]).toEqual({ from: 'vol1/king-#1.jpg', to: '001/001.jpg' });
  });

  it('says where a kept name lands when its volume is renamed', () => {
    const result = renumber(
      [added('book/vol1/001.jpg'), added('book/vol1/cover.png'), added('book/vol2/p1.jpg')],
      BOTH,
    );
    const firstVolume = result.groups.find(
      (group) => group.kind === 'pages' && group.folder === 'book/vol1',
    );
    expect(firstVolume?.renamed).toEqual([{ from: 'vol1/cover.png', to: '001/001_cover.png' }]);
    expect(firstVolume?.unchanged).toEqual([{ from: 'vol1/001.jpg', to: '001/001.jpg' }]);
    expect(unchangedLine(firstVolume!.unchanged[0])).toBe(
      'vol1/001.jpg → 001/001.jpg (name unchanged)',
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
