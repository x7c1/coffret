import { describe, expect, it } from 'vitest';

import { addedFile, type Added } from '@coffret/api';

import {
  corrected,
  editedPath,
  leftWholeLine,
  linesOf,
  misorderedLine,
  NO_EDITS,
  NO_NUMBERING,
  refusalLine,
  renumber,
  unchangedLine,
  type Edits,
  type Numbering,
} from './renumber';
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

  // Books write the second number of a spread without the frame's leading
  // text as often as with it. The `19` in `foobar19_` is text, not a number.
  it('numbers a spread whose second number drops the frame\'s leading text', () => {
    const book = (joined: string) =>
      paths([added('b/foobar19_0017.jpg'), added(`b/${joined}.jpg`), added('b/foobar19_0020.jpg')], PAGES);
    expect(book('foobar19_0018+0019')).toEqual(['b/017.jpg', 'b/018+019.jpg', 'b/020.jpg']);
    expect(book('foobar19_0018-0019')).toEqual(['b/017.jpg', 'b/018-019.jpg', 'b/020.jpg']);
    expect(book('foobar19_0018+foobar19_0019')).toEqual(['b/017.jpg', 'b/018+019.jpg', 'b/020.jpg']);
    expect(paths([added('b/p0017.jpg'), added('b/p0018+0019.jpg'), added('b/p0020.jpg')], PAGES)).toEqual([
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
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: { reason: 'same_number' } });
    expect(result.groups[0].unchanged).toHaveLength(3);
  });

  // `p2` beside `p2+p3` is page 2 scanned twice, which numbering would not
  // tell apart.
  it('leaves a group whole where a page carries a number of a spread', () => {
    const files = [added('b/p1.jpg'), added('b/p2.jpg'), added('b/p2+p3.jpg'), added('b/p4.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: { reason: 'same_number' } });
  });

  // `x.JPG` and `x.jpg` both become `002_x.jpg`, and the second to land would
  // replace the first.
  it('leaves a group whole where two names would be numbered to one', () => {
    const files = [added('b/p1.jpg'), added('b/p2.jpg'), added('b/x.JPG'), added('b/x.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({ renamed: [], leftWhole: { reason: 'taken' } });
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
    expect(pages?.leftWhole).toEqual({ reason: 'taken' });
  });

  it('leaves a group whole where a name comes before page 0', () => {
    const files = [added('b/cover.jpg'), added('b/p0.jpg'), added('b/p1.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0]).toMatchObject({
      renamed: [],
      leftWhole: { reason: 'order', earlier: 'cover.jpg', later: 'p0.jpg' },
    });
    expect(leftWholeLine(result.groups[0].leftWhole!)).toBe(
      'left as they are: numbering them would change the order they are read in ' +
        '(cover.jpg is read before p0.jpg)',
    );
  });

  it('numbers from page 0 where nothing comes before it', () => {
    expect(paths([added('b/p0.jpg'), added('b/p1.jpg'), added('b/x.jpg')], PAGES)).toEqual([
      'b/000.jpg',
      'b/001.jpg',
      'b/001_x.jpg',
    ]);
  });

  // `a2` and `a10` both go before page 1, and between themselves would sort
  // `000_a10` first by bytes; each takes its place among them as well.
  it('numbers names after one page in their place where bytes would sort them otherwise', () => {
    const files = [
      added('b/p1.jpg'),
      added('b/p2.jpg'),
      added('b/p3.jpg'),
      added('b/a2.jpg'),
      added('b/a10.jpg'),
    ];
    expect(paths(files, PAGES)).toEqual([
      'b/001.jpg',
      'b/002.jpg',
      'b/003.jpg',
      'b/000_1_a2.jpg',
      'b/000_2_a10.jpg',
    ]);
  });

  // By bytes `Credits` sorts before `cover`; read, it comes after.
  it('numbers names that differ in case after one page in the order they are read in', () => {
    const files = [added('b/cover.jpg'), added('b/Credits.jpg'), added('b/p001.jpg'), added('b/p002.jpg')];
    const sent = paths(files, PAGES);
    expect(sent).toEqual(['b/000_1_cover.jpg', 'b/000_2_Credits.jpg', 'b/001.jpg', 'b/002.jpg']);
    expect([...sent].sort(inLibraryOrder)).toEqual([
      'b/000_1_cover.jpg',
      'b/000_2_Credits.jpg',
      'b/001.jpg',
      'b/002.jpg',
    ]);
  });

  it('pads the place among names after one page alike', () => {
    const names = ['A', 'b', 'C', 'd', 'E', 'f', 'G', 'h', 'I', 'j'];
    const files = [...names.map((name) => added(`b/${name}.jpg`)), added('b/p1.jpg')];
    const sent = paths(files, PAGES);
    expect(sent.slice(0, 2)).toEqual(['b/000_01_A.jpg', 'b/000_02_b.jpg']);
    expect(sent[9]).toBe('b/000_10_j.jpg');
    expect([...sent].sort(inLibraryOrder)).toEqual(sent);
  });

  it('renames nothing where two frames are equally common', () => {
    const files = [added('b/a1.jpg'), added('b/a2.jpg'), added('b/b1.jpg'), added('b/b2.jpg')];
    const result = renumber(files, PAGES);
    expect(result.files.map((file) => file.path)).toEqual(files.map((file) => file.path));
    expect(result.groups[0].leftWhole).toEqual({ reason: 'tie' });
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

  // Readers look `ComicInfo.xml` up by its name, and a text beside the pages
  // is no page of the book.
  it('numbers images only, leaving every other file as it is', () => {
    const files = [
      added('b/ComicInfo.xml'),
      added('b/P001.JPG'),
      added('b/readme.txt'),
      added('b/P002.jpg'),
      added('b/P003.Png'),
    ];
    const result = renumber(files, PAGES);
    // Neither the cover-like `ComicInfo` nor `readme` goes before page 1, and
    // the width is the pages' own.
    expect(result.files.map((file) => file.path)).toEqual([
      'b/ComicInfo.xml',
      'b/001.jpg',
      'b/readme.txt',
      'b/002.jpg',
      'b/003.png',
    ]);
    expect(result.groups[0]).toMatchObject({ leftWhole: null });
    expect(result.groups[0].unchanged).toEqual([
      { from: 'ComicInfo.xml', to: 'ComicInfo.xml' },
      { from: 'readme.txt', to: 'readme.txt' },
    ]);
  });

  it('does not let a file that is no page change the width, the order or the numbers', () => {
    const files = [
      added('b/p1.jpg'),
      added('b/p2.jpg'),
      added('b/notes-1000.txt'),
      added('b/a10.xml'),
      added('b/p1.txt'),
    ];
    expect(paths(files, PAGES)).toEqual([
      'b/001.jpg',
      'b/002.jpg',
      'b/notes-1000.txt',
      'b/a10.xml',
      'b/p1.txt',
    ]);
  });

  it('judges no folder without an image as pages', () => {
    const result = renumber([added('b/a1.txt'), added('b/a2.txt')], PAGES);
    expect(result.files.map((file) => file.path)).toEqual(['b/a1.txt', 'b/a2.txt']);
    expect(result.groups).toEqual([]);
  });

  // `p0018-2` is a scanner's second take of page 18 as often as a spread; a
  // second number without the frame's text is a spread only where it is the
  // next page.
  it('reads a bare second number as a spread only where it is the next page', () => {
    expect(
      paths([added('b/p0017.jpg'), added('b/p0018.jpg'), added('b/p0018-2.jpg'), added('b/p0019.jpg')], PAGES),
    ).toEqual(['b/017.jpg', 'b/018.jpg', 'b/018_p0018-2.jpg', 'b/019.jpg']);
    expect(paths([added('b/p0017.jpg'), added('b/p0018+0019.jpg'), added('b/p0020.jpg')], PAGES)).toEqual([
      'b/017.jpg',
      'b/018+019.jpg',
      'b/020.jpg',
    ]);
    expect(
      paths([added('b/foobar19_0017.jpg'), added('b/foobar19_0018-0019.jpg'), added('b/foobar19_0020.jpg')], PAGES),
    ).toEqual(['b/017.jpg', 'b/018-019.jpg', 'b/020.jpg']);
    // Where the frame has no leading text to drop, the second number is bare.
    expect(paths([added('b/0017.jpg'), added('b/0018.jpg'), added('b/0018-2.jpg')], PAGES)).toEqual([
      'b/017.jpg',
      'b/018.jpg',
      'b/018_0018-2.jpg',
    ]);
    // Written with the full frame, it is taken as it is.
    expect(paths([added('b/p1.jpg'), added('b/p2+p4.jpg'), added('b/p5.jpg')], PAGES)).toEqual([
      'b/001.jpg',
      'b/002+004.jpg',
      'b/005.jpg',
    ]);
  });

  it('lines a group up in the order its names are read in, files that are no pages last', () => {
    const files = [
      added('b/notes.txt'),
      added('b/2.jpg'),
      added('b/03a.jpg'),
      added('b/001.jpg'),
      added('b/03.jpg'),
    ];
    const lines = linesOf(renumber(files, PAGES).groups[0]);
    expect(lines.map((line) => ('renamed' in line ? line.renamed.from : `=${line.unchanged.from}`))).toEqual([
      '=001.jpg',
      '2.jpg',
      '03.jpg',
      '03a.jpg',
      '=notes.txt',
    ]);
  });

  it('leaves files dropped loose alone', () => {
    expect(paths([added('p1.jpg'), added('p2.jpg'), added('b/p1.jpg')], PAGES)).toEqual([
      'p1.jpg',
      'p2.jpg',
      'b/001.jpg',
    ]);
  });
});

// The rule cannot read every book the way its owner meant, so a new name can be
// corrected before the drop is sent: the last component only, laid over the
// rule's names.
describe('correcting a new name', () => {
  /** `edits` with `path` corrected to `name`, failing where it is refused. */
  function correct(
    files: Added[],
    numbering: Numbering,
    edits: Edits,
    path: string,
    name: string | null,
  ): Edits {
    const result = corrected(files, numbering, edits, path, name);
    if ('refused' in result) {
      throw new Error(`refused: ${result.refused}`);
    }
    return result.edits;
  }

  /** Why `name` is not taken for `path`, or null where it is. */
  function refusal(files: Added[], numbering: Numbering, edits: Edits, path: string, name: string) {
    const result = corrected(files, numbering, edits, path, name);
    return 'refused' in result ? result.refused : null;
  }

  const BOOK = [added('b/p1.jpg'), added('b/p2.jpg'), added('b/p3.jpg')];

  it('sends a corrected page under its corrected name, and marks its line', () => {
    const edits = correct(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', '002a.jpg');
    const result = renumber(BOOK, PAGES, edits);
    expect(result.files.map((file) => file.path)).toEqual(['b/001.jpg', 'b/002a.jpg', 'b/003.jpg']);
    expect(result.groups[0].renamed[1]).toEqual({ from: 'p2.jpg', to: '002a.jpg', ruled: '002.jpg' });
    expect(editedPath(result.groups[0], result.groups[0].renamed[1])).toBe('b/p2.jpg');
  });

  it('moves every file under a corrected volume, in the paths and in the lines below it', () => {
    const files = [
      added('series/第1巻/p1.jpg'),
      added('series/第1巻/p2.jpg'),
      added('series/第2巻/p1.jpg'),
    ];
    const edits = correct(files, BOTH, NO_EDITS, 'series/第1巻', '001 first');
    const result = renumber(files, BOTH, edits);
    expect(result.files.map((file) => file.path)).toEqual([
      'series/001 first/001.jpg',
      'series/001 first/002.jpg',
      'series/002/001.jpg',
    ]);
    const pages = result.groups.find((group) => group.folder === 'series/第1巻');
    expect(pages?.renamed[0]).toEqual({ from: '第1巻/p1.jpg', to: '001 first/001.jpg' });
  });

  it('puts back the name the rule gave, and keeps no correction to the rule’s own name', () => {
    const edits = correct(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', 'two.jpg');
    expect(correct(BOOK, PAGES, edits, 'b/p2.jpg', null)).toEqual(NO_EDITS);
    expect(correct(BOOK, PAGES, edits, 'b/p2.jpg', '002.jpg')).toEqual(NO_EDITS);
  });

  it('refuses an empty name, a name with a separator, and a dot, each with its reason', () => {
    expect(refusal(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', '')).toBe('empty');
    expect(refusal(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', 'x/002.jpg')).toBe('separator');
    expect(refusal(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', '..')).toBe('dots');
    expect(refusalLine('empty')).toBe('a name cannot be empty');
    expect(refusalLine('separator')).toBe(
      'a name cannot contain /: only the name is corrected here, not where it lands',
    );
    expect(refusalLine('dots')).toBe('a name cannot be . or ..');
  });

  // Two files at one path would have the second replace the first, and a file
  // cannot share its path with a folder.
  it('refuses a name a sibling ends up with, among files, folders and corrections', () => {
    expect(refusal(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', '003.jpg')).toBe('taken');
    expect(refusalLine('taken')).toBe('there is already something with this name here');

    const once = correct(BOOK, PAGES, NO_EDITS, 'b/p1.jpg', 'one.jpg');
    expect(refusal(BOOK, PAGES, once, 'b/p2.jpg', 'one.jpg')).toBe('taken');
    // The name a correction gave up is free again.
    expect(refusal(BOOK, PAGES, once, 'b/p2.jpg', '001.jpg')).toBeNull();

    const beside = [...BOOK, added('b/scans/x.jpg')];
    expect(refusal(beside, PAGES, NO_EDITS, 'b/p2.jpg', 'scans')).toBe('taken');

    const volumes = [added('s/v1/x.jpg'), added('s/v2/x.jpg'), added('s/notes.txt')];
    expect(refusal(volumes, VOLUMES, NO_EDITS, 's/v1', '002')).toBe('taken');
    expect(refusal(volumes, VOLUMES, NO_EDITS, 's/v1', 'notes.txt')).toBe('taken');
    expect(refusal(volumes, VOLUMES, NO_EDITS, 's/v1', 'first')).toBeNull();
  });

  // Taken, since it may be what was meant, and said, since it may be a typo.
  it('takes a name that breaks the reading order and says so, and says nothing of one that keeps it', () => {
    const late = renumber(BOOK, PAGES, correct(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', '020.jpg'));
    expect(late.files.map((file) => file.path)).toEqual(['b/001.jpg', 'b/020.jpg', 'b/003.jpg']);
    expect(late.groups[0].renamed[1].misordered).toEqual({ sorts: 'after', neighbour: '003.jpg' });
    expect(misorderedLine(late.groups[0].renamed[1].misordered!)).toBe(
      'read out of order: this name sorts after 003.jpg',
    );

    const early = renumber(BOOK, PAGES, correct(BOOK, PAGES, NO_EDITS, 'b/p3.jpg', '000.jpg'));
    expect(early.groups[0].renamed[2].misordered).toEqual({ sorts: 'before', neighbour: '002.jpg' });

    const kept = renumber(BOOK, PAGES, correct(BOOK, PAGES, NO_EDITS, 'b/p2.jpg', '002a.jpg'));
    expect(kept.groups[0].renamed.map((rename) => rename.misordered)).toEqual([
      undefined,
      undefined,
      undefined,
    ]);
  });

  it('checks the order among volume folders too', () => {
    const files = [added('s/v1/x.jpg'), added('s/v2/x.jpg'), added('s/v3/x.jpg')];
    const result = renumber(files, VOLUMES, correct(files, VOLUMES, NO_EDITS, 's/v3', '0015'));
    expect(result.groups[0].renamed[2]).toMatchObject({
      to: '0015',
      misordered: { sorts: 'before', neighbour: '002' },
    });
  });
});
