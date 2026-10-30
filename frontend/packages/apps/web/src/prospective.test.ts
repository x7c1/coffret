import { expect, it, vi } from 'vitest';

import type { Listing } from '@coffret/api';

import {
  forgotten,
  prospectiveFolders,
  prospectiveIn,
  folderUnder,
  nameDefect,
  prepare,
  preparedAfter,
  takenIn,
} from './prospective';
import { destinationsOf, namedBy, newLine } from './destination';

function listing(over: Partial<Listing> = {}): Listing {
  return {
    path: '',
    mapped: true,
    held: true,
    folders: [],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
    ...over,
  };
}

// EP-2, EP-11, EP-14: a name the Library would refuse as a component of an
// Entry Path, or that coffret keeps for itself, is refused before anything is
// drawn for it.
it('refuses a name no drop could land under', () => {
  expect(nameDefect('')).toBe('a folder needs a name');
  expect(nameDefect('   ')).toBe('a folder needs a name');
  expect(nameDefect('comics/Series')).toContain('cannot hold a “/”');
  expect(nameDefect('..')).toContain('are not names');
  expect(nameDefect('.coffret')).toContain('.coffret');
  expect(nameDefect('.COFFRET')).toContain('only by case');
  expect(nameDefect('.coffret-fetch-x.part')).toContain('half written');
  expect(nameDefect('Series')).toBeNull();
  expect(nameDefect(' Series ')).toBeNull();
});

// EP-1: the path is held in the one spelling the server answers in.
it('composes the path in NFC, under the root without a separator', () => {
  expect(folderUnder('', 'comics')).toBe('comics');
  expect(folderUnder('comics', 'Séries')).toBe('comics/Séries');
});

// A name the folder already holds — a folder of the Library, a file, a folder
// on disk, a folder drawn as not held yet — is taken.
it('tells a name the folder already holds from one it does not', () => {
  const comics = listing({
    path: 'comics',
    folders: [{ name: 'Held', path: 'comics/Held', mapped: true }],
    files: [
      {
        name: 'cover.png',
        path: 'comics/cover.png',
        size: 1,
        mtime: null,
        deposited_at: null,
        state: 'added',
        container: null,
        reserved: false,
        openable: true,
        content_type: 'image/png',
      },
    ],
    folders_on_disk: ['OnDisk'],
  });
  expect(takenIn('Held', comics, [])).toBe(true);
  expect(takenIn('cover.png', comics, [])).toBe(true);
  expect(takenIn('OnDisk', comics, [])).toBe(true);
  expect(takenIn('Prepared', comics, ['comics/Prepared'])).toBe(true);
  expect(takenIn('Series', comics, [])).toBe(false);
  expect(takenIn('comics', listing({ empty_mapped_prefixes: ['comics'] }), [])).toBe(true);
});

it('prepares a path, or says why not and asks nothing it need not', async () => {
  const list = vi.fn((path: string) => Promise.resolve(listing({ path, folders_on_disk: ['taken'] })));
  for (const typed of ['', 'a/b', '.coffret']) {
    const answer = await prepare({ parent: 'comics', typed, prospective: [], list });
    expect(answer).toHaveProperty('refused');
  }
  expect(list).not.toHaveBeenCalled();

  expect(await prepare({ parent: 'comics', typed: 'taken', prospective: [], list })).toEqual({
    refused: 'there is already something called taken here',
  });
  expect(list).toHaveBeenCalledWith('comics');
  expect(await prepare({ parent: 'comics', typed: ' Series ', prospective: [], list })).toEqual({
    path: 'comics/Series',
  });

  // Nothing known about the place is not a place known to be free.
  const refused = await prepare({
    parent: '',
    typed: 'comics',
    prospective: [],
    list: () => Promise.reject(new Error('down')),
  });
  expect(refused).toHaveProperty('refused');
});

// EP-2: what the "New folder…" dialog says under its field, for each name the
// Library would refuse — and nothing for a name it takes.
it('refuses in the dialog an empty name, a `/`, `.`, `..` and a name already used', async () => {
  const list = (path: string) =>
    Promise.resolve(listing({ path, folders: [{ name: 'Vol. 01', path: 'comics/Vol. 01', mapped: true }] }));
  const said = async (typed: string) => {
    const answer = await prepare({ parent: 'comics', typed, prospective: [], list });
    return 'refused' in answer ? answer.refused : answer.path;
  };
  expect(await said('')).toBe('a folder needs a name');
  expect(await said('Vol. 02/extras')).toBe('a folder name cannot hold a “/” — prepare one folder at a time');
  expect(await said('.')).toBe('“.” and “..” are not names');
  expect(await said('..')).toBe('“.” and “..” are not names');
  expect(await said('Vol. 01')).toBe('there is already something called Vol. 01 here');
  expect(await said('Vol. 02')).toBe('comics/Vol. 02');
});

// A prepared path stays until the Library names it, and the list comes back as
// it was where nothing left it.
it('lets a prepared path go once the Library names it', () => {
  const held = ['comics', 'comics/Series'];
  expect(preparedAfter(held, () => false)).toBe(held);
  expect(preparedAfter(held, (path) => path === 'comics')).toEqual(['comics/Series']);
});

// A mistyped path is let go of from its own row, with every path prepared
// under it, and nothing else prepared goes with it.
it('forgets a prepared path and the paths prepared under it', () => {
  const held = ['books', 'comics/Seris', 'comics/Seris/Vol 1', 'comics/Series', 'comics/Seris 2'];
  expect(forgotten(held, 'comics/Seris')).toEqual(['books', 'comics/Series', 'comics/Seris 2']);
  expect(forgotten(held, 'albums')).toBe(held);
});

// The prepared paths and the empty mapped prefixes are one set of folders, in
// the Library's order, each once — and the latter only from the root's listing.
it('draws the prepared paths and the empty mapped prefixes together', () => {
  const root = listing({ empty_mapped_prefixes: ['comics', 'albums'] });
  expect(prospectiveFolders(['comics/Series', 'comics', 'books'], root, null)).toEqual([
    'albums',
    'books',
    'comics',
    'comics/Series',
  ]);
  expect(prospectiveFolders(['books'], null, null)).toEqual(['books']);
  expect(
    prospectiveFolders(['books'], listing({ path: 'albums', empty_mapped_prefixes: ['x'] }), null),
  ).toEqual(['books']);
});

// A root's listing read before a drop landed in an empty mapped prefix still
// names it; the tree's later answer holding it is what takes the mark away.
it('does not draw as not held yet a folder the tree already names', () => {
  const root = listing({ empty_mapped_prefixes: ['comics'] });
  expect(prospectiveFolders(['books'], root, ['comics', 'comics/Vol 1'])).toEqual(['books']);
  expect(prospectiveFolders(['books'], root, [])).toEqual(['books', 'comics']);
});

// The rows of one folder: those directly in it, reached where a mapping
// reaches — and none the listing names already.
it('gives the rows that stand directly in a folder', () => {
  const prospective = ['books', 'comics', 'comics/Series', 'comics/Series/Vol 1'];
  const root = listing({
    mapped: false,
    empty_mapped_prefixes: ['comics'],
    folders: [{ name: 'books', path: 'books', mapped: true, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null }],
  });
  expect(prospectiveIn(root, prospective)).toEqual([
    { name: 'comics', path: 'comics', mapped: true, present_throughout: false, files_total: 0, files_present: 0, size: 0, mtime: null },
  ]);
  expect(prospectiveIn(listing({ path: 'comics', held: false }), prospective)).toEqual([
    { name: 'Series', path: 'comics/Series', mapped: true, present_throughout: false, files_total: 0, files_present: 0, size: 0, mtime: null },
  ]);
});

// A drop into a prepared path is confirmed with the levels the Library does not
// have marked new, as every drop is: nothing tells the destination that a
// prepared folder exists.
it('leaves a prepared level new in a drop’s confirmation', () => {
  const where = listing({ path: 'comics/Series', held: false });
  const [destination] = destinationsOf(
    'comics/Series',
    { folders: [], loose: 3, looseBytes: 30, files: 3 },
    namedBy(['albums'], where),
  );
  expect(destination.levels.map((level) => level.state)).toEqual(['new', 'new']);
  expect(newLine(destination.levels)).toBe(
    'comics and Series are new — the Library has no such folders yet, and adding these files makes them',
  );
});
