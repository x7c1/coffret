import { describe, expect, it } from 'vitest';

import { addedFile, type Added, type Listing } from '@coffret/api';

import { destinationsOf, LIBRARY_ROOT, levelsOf, namedBy, newLine, pathLine } from './destination';
import { summarize } from './packChoice';

/** One dropped file at `path`, relative to the folder dropped onto. */
function added(path: string): Added {
  return addedFile(path, { size: 10, lastModified: 0 } as File);
}

/** Whether the Library names a folder, over the folders of a small Library. */
const NAMED = new Set(['comics', 'comics/Foo', 'comics/Foo/Vol 1']);
const exists = (path: string) => NAMED.has(path);

describe('levelsOf', () => {
  it('walks from the top of the Library down, saying which levels are there', () => {
    expect(levelsOf('comics/Foo/Vol 3', exists)).toEqual([
      { name: 'comics', path: 'comics', exists: true },
      { name: 'Foo', path: 'comics/Foo', exists: true },
      { name: 'Vol 3', path: 'comics/Foo/Vol 3', exists: false },
    ]);
    // The root is the Library itself, and no level.
    expect(levelsOf('', exists)).toEqual([]);
  });
});

describe('destinationsOf', () => {
  // A folder dropped onto a nested folder lands under its own name inside it,
  // and the level the drop makes is the one marked new.
  it('names a folder dropped onto a nested folder by its full path, and marks it new', () => {
    const summary = summarize([added('Vol 3/001.jpg'), added('Vol 3/002.jpg')], []);
    const [only, ...rest] = destinationsOf('comics/Foo', summary, exists);
    expect(rest).toEqual([]);
    expect(only.folder).toBe('Vol 3');
    expect(pathLine(only.levels)).toBe('comics/Foo/Vol 3/');
    expect(only.levels.map((level) => level.exists)).toEqual([true, true, false]);
    expect(newLine(only.levels)).toBe(
      'Vol 3 is new — the Library has no such folder yet, and this drop makes it',
    );
  });

  // Onto a folder the Library already has under that name, nothing is new.
  it('marks nothing new where the dropped folder is there already', () => {
    const summary = summarize([added('Vol 1/001.jpg')], ['Vol 1']);
    const [only] = destinationsOf('comics/Foo', summary, exists);
    expect(pathLine(only.levels)).toBe('comics/Foo/Vol 1/');
    expect(newLine(only.levels)).toBeNull();
  });

  // Onto the root, the dropped folder is a top-level folder of its own.
  it('names a folder dropped onto the root as a top-level folder', () => {
    const summary = summarize([added('manga/001.jpg')], []);
    const [only] = destinationsOf('', summary, exists);
    expect(pathLine(only.levels)).toBe('manga/');
    expect(newLine(only.levels)).toBe(
      'manga is new — the Library has no such folder yet, and this drop makes it',
    );
  });

  // Loose files land in the folder they were let go over — the root by name.
  it('puts loose files in the folder dropped onto, beside any dropped folder', () => {
    const alone = destinationsOf('', summarize([added('cover.png')], []), exists);
    expect(alone).toEqual([{ folder: null, levels: [] }]);
    expect(pathLine(alone[0].levels)).toBe(LIBRARY_ROOT);
    expect(newLine(alone[0].levels)).toBeNull();

    const both = destinationsOf(
      'comics/Foo',
      summarize([added('Vol 3/001.jpg'), added('cover.png')], []),
      exists,
    );
    expect(both.map((into) => [into.folder, pathLine(into.levels)])).toEqual([
      ['Vol 3', 'comics/Foo/Vol 3/'],
      [null, 'comics/Foo/'],
    ]);
  });

  // A folder the Library does not name yet — a book whose freeze has not
  // committed — is new however deep the drop goes into it.
  it('names every level the Library does not have yet', () => {
    const summary = summarize([added('ch-1/001.jpg')], []);
    const [only] = destinationsOf('comics/Bar', summary, exists);
    expect(pathLine(only.levels)).toBe('comics/Bar/ch-1/');
    expect(newLine(only.levels)).toBe(
      'Bar and ch-1 are new — the Library has no such folders yet, and this drop makes them',
    );
  });
});

describe('namedBy', () => {
  /** The listing of `comics/Foo`, held by the Library, with `Vol 1` in it. */
  const foo: Listing = {
    path: 'comics/Foo',
    mapped: true,
    held: true,
    folders: [{ name: 'Vol 1', path: 'comics/Foo/Vol 1', mapped: true }],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };

  it('reads the tree where it has answered', () => {
    const named = namedBy(['comics', 'comics/Bar'], { ...foo, held: false, folders: [] });
    expect(named('comics/Bar')).toBe(true);
    expect(named('comics/Foo')).toBe(false);
  });

  // The tree loading or failing must not turn every level into a new one, nor
  // call new a dropped folder the confirmation says already exists here.
  it('reads the listing of the folder let go over where the tree has not answered', () => {
    const summary = summarize([added('Vol 1/001.jpg'), added('Vol 3/001.jpg')], ['Vol 1']);
    const [one, three] = destinationsOf('comics/Foo', summary, namedBy(null, foo));
    expect(newLine(one.levels)).toBeNull();
    expect(three.levels.map((level) => level.exists)).toEqual([true, true, false]);
  });

  it('says nothing of a folder the listing holds nothing at', () => {
    const named = namedBy(null, { ...foo, held: false, folders: [] });
    expect(named('comics')).toBe(false);
    expect(named('comics/Foo')).toBe(false);
  });
});
