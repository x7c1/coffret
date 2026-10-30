import { describe, expect, it, vi } from 'vitest';

import { Refusal, type Browsed, type Listing, type Mapped } from '@coffret/api';

import {
  browseTo,
  chosen,
  chooseLine,
  chooseTitle,
  descend,
  folderToChoose,
  mapHere,
  noFolderChosenLine,
  pickerFor,
  toParent,
  typedOver,
  type Asking,
  type Picker,
} from './mapping';

function browsed(path: string, parent: string | null, names: string[] = []): Browsed {
  return {
    path,
    parent,
    folders: names.map((name) => ({ name, path: `${path === '/' ? '' : path}/${name}` })),
  };
}

const MAPPED: Mapped = {
  prefix: 'books',
  local_root: '/home/someone/books',
  replaced: null,
  marker: 'written',
  message: 'books is at /home/someone/books',
};

/** A picker held the way the component holds it, with what it asked recorded. */
function held(
  prefix: string | null,
  answers: {
    browse?: (path: string | null) => Promise<Browsed>;
    map?: (localRoot: string, prefix: string | null) => Promise<Mapped>;
  } = {},
) {
  let picker: Picker = pickerFor(prefix);
  const browse = vi.fn(answers.browse ?? ((path: string | null) => Promise.resolve(browsed(path ?? '/home/someone', '/home'))));
  const map = vi.fn(answers.map ?? (() => Promise.resolve(MAPPED)));
  const mapped = vi.fn();
  const asking: Asking = {
    browse,
    map,
    update: (change) => {
      picker = change(picker);
    },
    mapped,
  };
  return { asking, browse, map, mapped, now: () => picker };
}

// Descending: the folder chosen is listed, and its path is what the field holds.
it('goes into a folder and holds its path', async () => {
  const picking = held('books', {
    browse: (path) =>
      Promise.resolve(
        path === null
          ? browsed('/home/someone', '/home', ['scans'])
          : browsed(path, '/home/someone', []),
      ),
  });

  await browseTo(null, picking.asking);
  expect(picking.browse).toHaveBeenLastCalledWith(null);
  expect(picking.now().typed).toBe('/home/someone');

  const scans = picking.now().browsed?.folders[0];
  expect(scans?.name).toBe('scans');
  if (scans === undefined) {
    throw new Error('the home folder lists scans');
  }
  await descend(scans, picking.asking);
  expect(picking.browse).toHaveBeenLastCalledWith('/home/someone/scans');
  expect(picking.now().browsed?.path).toBe('/home/someone/scans');
  expect(picking.now().typed).toBe('/home/someone/scans');
});

// Going up: the parent the answer named is listed, and at the root of the
// filesystem there is nowhere to go and nothing is asked.
it('goes to the parent, and stays put at the root', async () => {
  const picking = held(null);
  await browseTo('/home/someone', picking.asking);

  await toParent(picking.now(), picking.asking);
  expect(picking.browse).toHaveBeenLastCalledWith('/home');

  const atRoot = held(null, { browse: () => Promise.resolve(browsed('/', null, ['home'])) });
  await browseTo('/', atRoot.asking);
  await toParent(atRoot.now(), atRoot.asking);
  expect(atRoot.browse).toHaveBeenCalledTimes(1);
});

// A path typed over the browsed one is what is mapped, and the prefix is the
// banner's, carried through untouched.
it('maps a typed path in place of the browsed one, to the banner’s prefix', async () => {
  const picking = held('books');
  await browseTo(null, picking.asking);
  picking.asking.update((picker) => typedOver(picker, '  /mnt/scans/books  '));
  expect(chosen(picking.now())).toBe('/mnt/scans/books');

  await mapHere(picking.now(), picking.asking);
  expect(picking.map).toHaveBeenCalledWith('/mnt/scans/books', 'books');
  expect(picking.mapped).toHaveBeenCalledWith(MAPPED);
  expect(picking.now().busy).toBe(false);
});

// The Library root is mapped with no prefix at all, which is `null`.
it('maps the Library root with no prefix', async () => {
  const picking = held(null);
  await browseTo(null, picking.asking);
  await mapHere(picking.now(), picking.asking);
  expect(picking.map).toHaveBeenCalledWith('/home/someone', null);
});

// A refusal is the picker's line, and the picker stays: nothing is handed on,
// and what was browsed and typed is still there to correct.
it('keeps a refusal in the picker and stays open', async () => {
  const picking = held('books', {
    map: () =>
      Promise.reject(
        new Refusal('bad_request', 400, '/home/someone/nowhere is not a directory on this device'),
      ),
  });
  await browseTo(null, picking.asking);
  picking.asking.update((picker) => typedOver(picker, '/home/someone/nowhere'));

  await mapHere(picking.now(), picking.asking);
  expect(picking.mapped).not.toHaveBeenCalled();
  expect(picking.now().refused).toBe('/home/someone/nowhere is not a directory on this device');
  expect(picking.now().typed).toBe('/home/someone/nowhere');
  expect(picking.now().browsed?.path).toBe('/home/someone');

  // And a browse that is refused keeps the folder shown before it.
  const browsing = held('books', {
    browse: (path) =>
      path === null
        ? Promise.resolve(browsed('/home/someone', '/home'))
        : Promise.reject(new Refusal('bad_request', 403, `${path} cannot be read`)),
  });
  await browseTo(null, browsing.asking);
  await browseTo('/root', browsing.asking);
  expect(browsing.now().refused).toBe('/root cannot be read');
  expect(browsing.now().browsed?.path).toBe('/home/someone');
});

// An empty field maps nothing, and says what to do.
it('asks for a folder rather than mapping nothing', async () => {
  const picking = held('books');
  await mapHere(picking.now(), picking.asking);
  expect(picking.map).not.toHaveBeenCalled();
  expect(picking.now().refused).toBe('choose a folder, or type its whole path');
});

// What the step asks names the part of the Library the folder is for, and
// says how far the folder reaches — never the word "map".
it('asks for a folder for a top-level folder or for the whole Library, in the same terms as the banner', () => {
  expect(chooseTitle('books')).toBe('Choose a folder on this device for “books”');
  expect(chooseTitle(null)).toBe('Choose a folder on this device for the Library');
  expect(chooseLine('books')).toBe(
    'Files added are kept there as well as in the Library, and everything under “books” uses it.',
  );
  expect(chooseLine(null)).toContain('everything in the Library uses it');
  expect(noFolderChosenLine('books')).toBe('No folder chosen for “books” yet');
  for (const said of [chooseTitle('books'), chooseLine(null), noFolderChosenLine(null)]) {
    expect(said).not.toMatch(/\bmap/i);
    expect(said).not.toContain('not on this device');
  }
});

/**
 * The listing of a Library root: `books` with a folder of this device chosen
 * for it or not, `albums` without, and the whole Library chosen for or not.
 */
function rootListing({ books, library }: { books: boolean; library: boolean }): Listing {
  return {
    path: '',
    // A folder's own answer repeats the root's where one is chosen for the
    // whole Library, as the server says it.
    mapped: library,
    held: true,
    folders: [
      { name: 'books', path: 'books', mapped: books || library, present_throughout: false, files_total: 1, files_present: 0, size: 1, mtime: null },
      { name: 'albums', path: 'albums', mapped: library, present_throughout: false, files_total: 1, files_present: 0, size: 1, mtime: null },
    ],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
}

// EP-9: adding needs a folder on this device for the destination's top-level
// folder or for the whole Library, and is asked for only where neither exists.
describe('when the folder-on-this-device step appears', () => {
  it('asks where neither the top-level folder nor the whole Library has one', () => {
    const none = rootListing({ books: false, library: false });
    expect(folderToChoose('books/Foo', none)).toEqual({ prefix: 'books' });
    expect(folderToChoose('books', none)).toEqual({ prefix: 'books' });
  });

  it('does not ask where the top-level folder has one', () => {
    expect(folderToChoose('books/Foo', rootListing({ books: true, library: false }))).toBeNull();
    // Nor for a top-level folder chosen for with nothing in it yet.
    const empty = { ...rootListing({ books: false, library: false }), empty_mapped_prefixes: ['comics'] };
    expect(folderToChoose('comics/Series', empty)).toBeNull();
  });

  it('does not ask where only the whole Library has one', () => {
    const library = rootListing({ books: false, library: true });
    expect(folderToChoose('books/Foo', library)).toBeNull();
    expect(folderToChoose('', library)).toBeNull();
  });

  it('asks for the whole Library at the root, whatever top-level folders have one', () => {
    // The server takes an add at the root only through the whole Library's
    // folder, so a top-level folder's own does not let it through (spec: EP-9).
    expect(folderToChoose('', rootListing({ books: true, library: false }))).toEqual({ prefix: null });
    expect(folderToChoose('', rootListing({ books: false, library: false }))).toEqual({ prefix: null });
  });
});
