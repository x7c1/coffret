import { describe, expect, it, vi } from 'vitest';

import { addedFile, type Added, type ContainerKind, type Listing } from '@coffret/api';

import {
  choiceLabels,
  chooseAndAdd,
  collisionsOf,
  existsLine,
  folderLine,
  heldIn,
  heldUnder,
  looseLine,
  NOTHING_HELD,
  questionOf,
  refusedLine,
  replacesLine,
  summarize,
  type Choice,
  type Choosing,
  type HeldFiles,
} from './packChoice';
import { NO_EDITS, NO_NUMBERING, type Edits, type Numbering } from './renumber';

/** One dropped file of `size` bytes at `path`, relative to the folder dropped onto. */
function added(path: string, size = 10): Added {
  // Only the size is read of a file before anything is sent, so a stand-in
  // holding just that is the file as far as this module is concerned — which
  // is what lets a case speak of a gigabyte without allocating one.
  return addedFile(path, { size, lastModified: 0 } as File);
}

/** A book of `pages` pages of `size` bytes each under `folder`, some a level down. */
function book(folder: string, pages: number, size = 10): Added[] {
  return Array.from({ length: pages }, (_, at) =>
    added(at % 2 === 0 ? `${folder}/page-${at}.jpg` : `${folder}/scans/page-${at}.jpg`, size),
  );
}

describe('summarize', () => {
  // The count is every file at every depth under the dropped folder, so the
  // number shown is the number of parts the request will carry.
  it('counts every file under a dropped folder, however deep', () => {
    const files = [...book('BookX', 80, 725_000), added('BookX/a/b/c/deep.jpg', 0)];
    const summary = summarize(files, []);
    expect(summary.folders).toEqual([
      { name: 'BookX', files: 81, bytes: 58_000_000, exists: false },
    ]);
    expect(summary.files).toBe(files.length);
    expect(folderLine(summary.folders[0])).toBe('📁 BookX — 81 files, 58.0 MB');
  });

  // Files alone are confirmed too: how many and how much, and that they go
  // one by one, since there is no folder to make a Pack of.
  it('counts a drop of files alone, and says they are added one by one', () => {
    const summary = summarize([added('cover.png', 1_500_000), added('notes.txt', 500_000)], []);
    expect(summary).toEqual({ folders: [], loose: 2, looseBytes: 2_000_000, files: 2 });
    expect(questionOf(summary)).toBe('this drop holds 2 files — add them to the Library?');
    expect(looseLine(summary)).toBe('2 files, 2.0 MB — added one by one');

    const one = summarize([added('cover.png', 1_000)], []);
    expect(questionOf(one)).toBe('this drop holds 1 file — add it to the Library?');
    expect(looseLine(one)).toMatch(/^1 file, .* — added on its own$/);
  });

  // Several folders are listed each with its own totals, and loose files beside
  // them are counted into the drop the answer applies to.
  it('lists each folder of a drop and the loose files beside them', () => {
    const files = [...book('vol-1', 3), ...book('vol-2', 2), added('cover.png')];
    const summary = summarize(files, ['vol-2']);
    expect(summary.folders.map((folder) => [folder.name, folder.files, folder.exists])).toEqual([
      ['vol-1', 3, false],
      ['vol-2', 2, true],
    ]);
    expect(summary.loose).toBe(1);
    expect(summary.files).toBe(6);
    expect(existsLine(summary.folders[1])).toMatch(/^vol-2 already exists here — /);
    expect(questionOf(summary)).toBe('this drop holds 2 folders — how should it go into the Library?');
    expect(looseLine(summary)).toBe('and 1 file beside them, added the same way');
    expect(choiceLabels(summary)).toEqual({
      pack: 'As a Pack (recommended)',
      one_by_one: 'The 6 files one by one',
    });
  });
});

describe('chooseAndAdd', () => {
  /**
   * A drop that answers the question with `choice` and `numbering`, recording
   * what it was asked and the paths it sent.
   */
  function choosing(
    files: Added[],
    choice: Choice = 'pack',
    numbering: Numbering = NO_NUMBERING,
    edits: Edits = NO_EDITS,
  ) {
    const order: string[] = [];
    const sent: string[][] = [];
    const asking: Choosing = {
      files,
      existing: [],
      held: vi.fn(async () => NOTHING_HELD),
      ask: vi.fn(async () => {
        order.push('asked');
        return choice === 'cancel' ? choice : { choice, numbering, edits };
      }),
      send: vi.fn(async (sending: readonly Added[], freeze: boolean) => {
        order.push(freeze ? 'sent as a Pack' : 'sent one by one');
        sent.push(sending.map((file) => file.path));
      }),
      refuse: vi.fn(),
    };
    return { asking, order, sent };
  }

  it('asks about a drop holding a folder before anything is sent', async () => {
    const { asking, order } = choosing(book('BookX', 4));
    expect(await chooseAndAdd(asking)).toBe('sent');
    expect(order).toEqual(['asked', 'sent as a Pack']);
    expect(asking.ask).toHaveBeenCalledWith(
      expect.objectContaining({ files: 4, folders: [expect.objectContaining({ files: 4 })] }),
      asking.files,
      NOTHING_HELD,
    );
  });

  // What the Library holds where the drop lands is looked up before the
  // question goes up, and handed to it.
  it('hands the question what the Library holds where the drop lands', async () => {
    const { asking } = choosing(book('BookX', 2));
    const held: HeldFiles = new Map([['BookX/page-0.jpg', 'one-file']]);
    asking.held = vi.fn(async () => held);
    await chooseAndAdd(asking);
    expect(asking.held).toHaveBeenCalledWith(asking.files);
    expect(asking.ask).toHaveBeenCalledWith(expect.anything(), asking.files, held);
  });

  // A drop of files alone is confirmed like any other, and goes one by one
  // under the names it was dropped with — a Pack or a numbering it could not
  // have been offered changes neither.
  it('asks about a drop of files alone before sending it one by one', async () => {
    const files = [added('cover.png'), added('king-#1.jpg')];
    for (const choice of ['one_by_one', 'pack'] as const) {
      const { asking, order, sent } = choosing(files, choice, { pages: true, volumes: true });
      expect(await chooseAndAdd(asking)).toBe('sent');
      expect(asking.held).toHaveBeenCalledWith(files);
      expect(asking.ask).toHaveBeenCalledWith(
        expect.objectContaining({ folders: [], loose: 2 }),
        files,
        NOTHING_HELD,
      );
      expect(order).toEqual(['asked', 'sent one by one']);
      expect(sent).toEqual([['cover.png', 'king-#1.jpg']]);
    }
  });

  it('sends nothing of a drop of files alone when it is cancelled', async () => {
    const { asking } = choosing([added('cover.png')], 'cancel');
    expect(await chooseAndAdd(asking)).toBe('cancelled');
    expect(asking.send).not.toHaveBeenCalled();
  });

  // Choosing Pack asks the server for a freeze; choosing one by one asks for
  // the sync, as a drop of files does.
  it('sends as a Pack or one by one as the person chose', async () => {
    const pack = choosing(book('BookX', 2), 'pack');
    await chooseAndAdd(pack.asking);
    expect(pack.asking.send).toHaveBeenCalledWith(expect.anything(), true);

    const loose = choosing(book('BookX', 2), 'one_by_one');
    await chooseAndAdd(loose.asking);
    expect(loose.asking.send).toHaveBeenCalledWith(expect.anything(), false);
  });

  // The names are chosen in the question and the request is what carries
  // them, whichever way the drop is added.
  it.each(['pack', 'one_by_one'] as const)(
    'sends the numbered paths when asked to, and the dropped ones when not (%s)',
    async (choice) => {
      const files = [added('BookX/king-#1.jpg'), added('BookX/king-#2.jpg'), added('BookX/cover.png')];

      const numbered = choosing(files, choice, { pages: true, volumes: false });
      expect(await chooseAndAdd(numbered.asking)).toBe('sent');
      expect(numbered.asking.ask).toHaveBeenCalledWith(expect.anything(), files, NOTHING_HELD);
      expect(numbered.sent).toEqual([['BookX/001.jpg', 'BookX/002.jpg', 'BookX/000_cover.png']]);

      const asDropped = choosing(files, choice, NO_NUMBERING);
      await chooseAndAdd(asDropped.asking);
      expect(asDropped.sent).toEqual([files.map((file) => file.path)]);
    },
  );

  // A new name the person corrected is the name the file goes up under: the
  // answer carries the corrections, and the drop is numbered with them.
  it('sends the corrected paths the answer carries', async () => {
    const files = [
      added('BookX/vol1/king-#1.jpg'),
      added('BookX/vol1/king-#2.jpg'),
      added('BookX/vol2/king-#1.jpg'),
    ];
    const edits: Edits = new Map([
      ['BookX/vol1', 'first'],
      ['BookX/vol1/king-#2.jpg', 'two.jpg'],
    ]);
    const { asking, sent } = choosing(files, 'pack', { pages: true, volumes: true }, edits);
    expect(await chooseAndAdd(asking)).toBe('sent');
    expect(sent).toEqual([['BookX/first/001.jpg', 'BookX/first/two.jpg', 'BookX/002/001.jpg']]);
  });

  it('sends nothing when the question is cancelled', async () => {
    const { asking } = choosing(book('BookX', 2), 'cancel');
    expect(await chooseAndAdd(asking)).toBe('cancelled');
    expect(asking.send).not.toHaveBeenCalled();
    expect(asking.refuse).not.toHaveBeenCalled();
  });

  // LA-9: a drop the server is certain to refuse is refused here, before the
  // question — a choice about how to add what cannot be added is no choice.
  it('refuses a drop past a budget without asking about it', async () => {
    const { asking } = choosing([...book('BookX', 2), added('BookX/huge.tif', 2 ** 31)]);
    expect(await chooseAndAdd(asking)).toBe('refused');
    expect(asking.ask).not.toHaveBeenCalled();
    expect(asking.send).not.toHaveBeenCalled();
    expect(asking.refuse).toHaveBeenCalledWith(
      expect.objectContaining({ budget: 'part', name: 'BookX/huge.tif' }),
    );
  });
});

/** What the Library holds at each of `paths`, relative to the folder dropped onto. */
function holding(entries: Record<string, ContainerKind>): HeldFiles {
  return new Map(Object.entries(entries));
}

// PK-10: a dropped file whose path holds a one-file Entry replaces it, and one
// whose path a Pack holds is refused. The question says which, by name.
describe('collisions', () => {
  it('says nothing more where no dropped path is held', () => {
    const collisions = collisionsOf(book('BookX', 3), holding({ 'BookX/other.jpg': 'one-file' }));
    expect(collisions).toEqual({ replaced: [], refused: [] });
    expect(replacesLine(collisions)).toBeNull();
    expect(refusedLine(collisions)).toBeNull();
  });

  it('names the files that replace one-file Entries, the first five and a count of the rest', () => {
    const files = Array.from({ length: 7 }, (_, at) => added(`BookX/p${at}.jpg`));
    const held = holding(Object.fromEntries(files.map((file) => [file.path, 'one-file' as const])));
    const collisions = collisionsOf(files, held);
    expect(collisions.refused).toEqual([]);
    expect(replacesLine(collisions)).toBe(
      '7 files in this drop replace the files the Library already has at the same path: ' +
        'BookX/p0.jpg, BookX/p1.jpg, BookX/p2.jpg, BookX/p3.jpg, BookX/p4.jpg, and 2 more',
    );
    expect(refusedLine(collisions)).toBeNull();

    expect(replacesLine(collisionsOf([files[0]], held))).toBe(
      '1 file in this drop replaces the file the Library already has at the same path: BookX/p0.jpg',
    );
  });

  it('counts the files a Pack refuses beside the ones that are replaced', () => {
    const files = [added('BookX/a.jpg'), added('BookX/b.jpg'), added('BookX/c.jpg'), added('loose.txt')];
    const collisions = collisionsOf(
      files,
      holding({ 'BookX/a.jpg': 'one-file', 'BookX/b.jpg': 'pack', 'BookX/c.jpg': 'pack' }),
    );
    expect(collisions).toEqual({ replaced: ['BookX/a.jpg'], refused: ['BookX/b.jpg', 'BookX/c.jpg'] });
    expect(replacesLine(collisions)).toContain('BookX/a.jpg');
    expect(refusedLine(collisions)).toBe(
      '2 files are refused, because a Pack holds the file at that path',
    );
  });
});

describe('heldUnder', () => {
  /** A listing of `path` holding `files` (name → kind) and `folders`. */
  function listingOf(
    path: string,
    files: Record<string, ContainerKind | null>,
    folders: string[] = [],
  ): Listing {
    const at = (name: string) => (path === '' ? name : `${path}/${name}`);
    return {
      path,
      mapped: true,
      held: true,
      folders: folders.map((name) => ({ name, path: at(name), mapped: true })),
      files: Object.entries(files).map(([name, container]) => ({
        name,
        path: at(name),
        size: 1,
        mtime: null,
        deposited_at: null,
        state: container === null ? 'added' : 'remote',
        container,
        reserved: false,
        openable: true,
        content_type: 'image/jpeg',
      })),
      folders_on_disk: [],
      empty_mapped_prefixes: [],
    };
  }

  // The listing on hand answers for the loose files; a folder the drop carries
  // is listed, and one under it the drop carries too, and nothing it does not.
  it('lists only the folders the drop reaches, and keys what they hold by the dropped path', async () => {
    const landing = listingOf('albums', { 'cover.png': 'one-file', 'new.png': null }, [
      'BookX',
      'Other',
    ]);
    const listings: Record<string, Listing> = {
      'albums/BookX': listingOf('albums/BookX', { 'p1.jpg': 'pack' }, ['scans', 'unrelated']),
      'albums/BookX/scans': listingOf('albums/BookX/scans', { 'p2.jpg': 'one-file' }),
    };
    const list = vi.fn(async (folder: string) => listings[folder]);

    const held = await heldUnder(
      landing,
      [added('cover.png'), added('new.png'), added('BookX/p1.jpg'), added('BookX/scans/p2.jpg')],
      list,
    );

    expect(list.mock.calls.map(([folder]) => folder).sort()).toEqual([
      'albums/BookX',
      'albums/BookX/scans',
    ]);
    expect(held).toEqual(
      holding({ 'cover.png': 'one-file', 'BookX/p1.jpg': 'pack', 'BookX/scans/p2.jpg': 'one-file' }),
    );
  });

  // At the Library root the dropped paths are the Library's own.
  it('reads a drop onto the Library root', async () => {
    const landing = listingOf('', {}, ['BookX']);
    const list = vi.fn(async () => listingOf('BookX', { 'p1.jpg': 'one-file' }));
    expect(await heldUnder(landing, [added('BookX/p1.jpg')], list)).toEqual(
      holding({ 'BookX/p1.jpg': 'one-file' }),
    );
    expect(list).toHaveBeenCalledWith('BookX');
  });

  // A corrected volume name sends files into a folder no numbering would, and
  // what is held there is asked for by the folders the corrected drop reaches.
  it('lists the folders it is given, as far as the Library has them', async () => {
    const landing = listingOf('albums', {}, ['BookX']);
    const listings: Record<string, Listing> = {
      'albums/BookX': listingOf('albums/BookX', {}, ['first']),
      'albums/BookX/first': listingOf('albums/BookX/first', { '001.jpg': 'pack' }),
    };
    const list = vi.fn(async (folder: string) => listings[folder]);
    expect(await heldIn(landing, ['BookX', 'BookX/first', 'BookX/second'], list)).toEqual(
      holding({ 'BookX/first/001.jpg': 'pack' }),
    );
    expect(list.mock.calls.map(([folder]) => folder).sort()).toEqual([
      'albums/BookX',
      'albums/BookX/first',
    ]);
  });

  it('asks for nothing where no dropped folder is already there', async () => {
    const list = vi.fn();
    expect(await heldUnder(listingOf('albums', {}, ['Other']), book('BookX', 2), list)).toEqual(
      NOTHING_HELD,
    );
    expect(list).not.toHaveBeenCalled();
  });
});
