// A drop in the desktop app's window: the files are handed to the app's bridge
// by path, the records the explorer builds from its answer go through the same
// confirmation and Pack choice a browser's drop does, and they are sent as an
// import naming the handle rather than as bytes.

import { addFiles, dropToken, holdToken, type Added } from '@coffret/api';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  desktopBridge,
  droppedFiles,
  filesUnder,
  heldBy,
  nothingAddedLine,
  NothingTaken,
  pickedFiles,
  notTakenLine,
  notTakenNotice,
  withNotTaken,
  type DesktopBridge,
  type DroppedFolder,
  type HeldDrop,
} from './drop';
import { overBudgetLine } from './dropped';
import { chooseAndAdd, NOTHING_HELD } from './packChoice';

const HANDLE = 'd'.repeat(64);

/** What the app answers a drop of one folder holding a three-page book with. */
const BOOK: HeldDrop = {
  handle: HANDLE,
  files: [1, 2, 3].map((page) => ({
    path: `vol-1/page-${page}.jpg`,
    size: 100,
    mtime: 1_444_000_000_000,
  })),
  notTaken: [],
};

/** A transfer carrying `files` and, as a browser's does, an entry for each. */
function transfer(files: File[]): DataTransfer {
  return {
    files,
    items: files.map((file) => ({
      webkitGetAsEntry: () => ({
        isFile: true,
        isDirectory: false,
        name: file.name,
        file: (onFile: (dropped: File) => void) => onFile(file),
      }),
    })),
  } as unknown as DataTransfer;
}

/** Every request the page made, by path, and the body it carried. */
function recording(): { path: string; body: unknown }[] {
  const made: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
    const body = init?.body instanceof Blob ? JSON.parse(await textOf(init.body)) : init?.body;
    made.push({ path: new URL(url, window.location.origin).pathname, body });
    return Response.json({ written: ['books/vol-1/page-1.jpg'], refused: [] });
  });
  return made;
}

/** A body as text, read the way jsdom's `Blob`, which has no `text`, allows. */
function textOf(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsText(blob);
  });
}

beforeEach(() => {
  holdToken('the-token-this-tab-holds');
});

afterEach(() => {
  vi.unstubAllGlobals();
  dropToken();
});

describe('with the desktop app’s bridge', () => {
  it('builds the drop’s records from what the app answered', async () => {
    const takeDrop = vi.fn<DesktopBridge['takeDrop']>(async () => BOOK);
    vi.stubGlobal('coffretDesktop', { takeDrop });
    const dropped = new File(['folder'], 'vol-1');

    const drop = await droppedFiles(transfer([dropped]));

    expect(takeDrop).toHaveBeenCalledWith([dropped]);
    expect(drop.files).toEqual(
      BOOK.files.map((file, index) => ({
        path: file.path,
        size: 100,
        mtime: 1_444_000_000_000,
        source: { kind: 'held', handle: HANDLE, index },
      })),
    );
    expect(drop.notTaken).toEqual([]);
  });

  it('goes through the same Pack choice and is sent as an import', async () => {
    vi.stubGlobal('coffretDesktop', { takeDrop: async () => BOOK });
    const made = recording();
    const { files: added } = await droppedFiles(transfer([new File(['folder'], 'vol-1')]));
    const asked = vi.fn(async () => ({
      choice: 'pack' as const,
      numbering: { pages: true, volumes: false },
    }));

    const chosen = await chooseAndAdd({
      files: added,
      existing: [],
      held: async () => NOTHING_HELD,
      ask: asked,
      send: async (sending: readonly Added[], freeze: boolean) => {
        await addFiles('books', sending, { freeze });
      },
      refuse: () => {
        throw new Error('a book of three pages is within every budget');
      },
    });

    expect(chosen).toBe('sent');
    const [summary] = asked.mock.calls[0] as unknown as [{ folders: { name: string; files: number }[] }];
    expect(summary.folders).toMatchObject([{ name: 'vol-1', files: 3 }]);
    expect(made).toHaveLength(1);
    expect(made[0].path).toBe('/api/import');
    expect(made[0].body).toEqual({
      path: 'books',
      freeze: true,
      handle: HANDLE,
      // Renumbered as the person asked, and each still named by its index in
      // what the app holds.
      files: [
        { index: 0, name: 'vol-1/001.jpg' },
        { index: 1, name: 'vol-1/002.jpg' },
        { index: 2, name: 'vol-1/003.jpg' },
      ],
    });
  });

  it('says why nothing was added where the app took nothing', async () => {
    const bridge: DesktopBridge = {
      takeDrop: async () => ({
        handle: null,
        files: [],
        notTaken: [{ path: 'link-to-photos', reason: 'not_a_file_or_folder' }],
      }),
    };

    await expect(heldBy(bridge, [new File([''], 'link-to-photos')])).rejects.toThrow(
      'link-to-photos is a link',
    );
    await expect(heldBy(bridge, [])).resolves.toEqual({ files: [], notTaken: [], systemFiles: [] });
  });

  it('adds nothing and says nothing of a drop that had no paths at all', async () => {
    // An image dragged out of another page: the app is handed an empty path.
    const bridge: DesktopBridge = {
      takeDrop: async () => ({
        handle: null,
        files: [],
        notTaken: [{ path: '', reason: 'empty' }],
      }),
    };

    await expect(heldBy(bridge, [new File(['an image'], 'image.png')])).resolves.toEqual({
      files: [],
      notTaken: [],
      systemFiles: [],
    });
  });

  // EP-17: what the app left out because an operating system keeps it is
  // taken out of what was not taken, so the warning never names it, and it is
  // handed back apart — also where it is all the drop held.
  it('hands what the system keeps back apart from what was not taken', async () => {
    const bridge: DesktopBridge = {
      takeDrop: async () => ({
        ...BOOK,
        notTaken: [
          { path: 'vol-1/.DS_Store', reason: 'system_file' },
          { path: 'vol-1/__MACOSX', reason: 'system_folder' },
          LINK,
        ],
      }),
    };

    const drop = await heldBy(bridge, [new File([''], 'vol-1')]);
    expect(drop.notTaken).toEqual([LINK]);
    expect(drop.systemFiles).toEqual([
      { path: 'vol-1/.DS_Store', folder: false },
      { path: 'vol-1/__MACOSX', folder: true },
    ]);

    const only: DesktopBridge = {
      takeDrop: async () => ({
        handle: null,
        files: [],
        notTaken: [{ path: '.DS_Store', reason: 'system_file' }],
      }),
    };
    await expect(heldBy(only, [new File([''], '.DS_Store')])).resolves.toEqual({
      files: [],
      notTaken: [],
      systemFiles: [{ path: '.DS_Store', folder: false }],
    });
  });

  it('names what had a path, not the empty ones, where nothing was taken', async () => {
    const bridge: DesktopBridge = {
      takeDrop: async () => ({
        handle: null,
        files: [],
        notTaken: [
          { path: '', reason: 'empty' },
          { path: 'link-to-photos', reason: 'not_a_file_or_folder' },
        ],
      }),
    };

    await expect(heldBy(bridge, [new File([''], 'a'), new File([''], 'b')])).rejects.toThrow(
      /^link-to-photos is a link or something else that is neither a file nor a folder$/,
    );
  });
});

const LINK = { path: 'link-to-photos', reason: 'not_a_file_or_folder' } as const;

/** What the app answers a drop of three photos with a symbolic link beside them. */
const PHOTOS_AND_A_LINK: HeldDrop = {
  handle: HANDLE,
  files: ['a.jpg', 'b.jpg', 'c.jpg'].map((path) => ({ path, size: 100, mtime: 0 })),
  notTaken: [
    // An image dragged in beside them from another page, which has no path to name.
    { path: '', reason: 'empty' },
    LINK,
  ],
};

const LINK_LINE =
  '1 thing was not taken: link-to-photos is a link or something else that is neither a file nor a folder';

describe('a drop the app took only part of', () => {
  it('carries what was not taken beside what was, leaving out what had no path', async () => {
    vi.stubGlobal('coffretDesktop', { takeDrop: async () => PHOTOS_AND_A_LINK });

    const drop = await droppedFiles(transfer([new File([''], 'a.jpg')]));

    expect(drop.files.map((added) => added.path)).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
    expect(drop.notTaken).toEqual([LINK]);
    expect(notTakenNotice(drop.notTaken)).toBe(LINK_LINE);
  });

  it('says it beside the reason where the drop is answered without the confirmation', async () => {
    vi.stubGlobal('coffretDesktop', { takeDrop: async () => PHOTOS_AND_A_LINK });
    const { files, notTaken } = await droppedFiles(transfer([new File([''], 'a.jpg')]));
    // More files than one request may name: refused without a question.
    const many = Array.from({ length: 70_001 }, () => files[0]);
    const said: string[] = [];

    const chosen = await chooseAndAdd({
      files: many,
      existing: [],
      held: async () => NOTHING_HELD,
      ask: () => {
        throw new Error('a drop that cannot be sent is not asked about');
      },
      send: async () => {
        throw new Error('a drop that cannot be sent is not sent');
      },
      refuse: (overdrawn) => said.push(withNotTaken(overBudgetLine(overdrawn, { name: 'main', library_id: '0123456789abcdef' }), notTaken)),
    });

    expect(chosen).toBe('refused');
    expect(said).toHaveLength(1);
    expect(said[0]).toMatch(/^these are 70001 files, more than the \d+ that can be added at once\. /);
    // A sentence of its own, after the advice the refusal ends on rather than
    // part of it.
    expect(said[0].endsWith(`main\`. ${LINK_LINE}.`)).toBe(true);
  });

  it('says it as the next clause after a one-clause notice', () => {
    expect(withNotTaken('nothing was added — x', [LINK])).toBe(`nothing was added — x; ${LINK_LINE}`);
  });

  it('says nothing more of a drop the app took all of, or of a browser’s', () => {
    expect(notTakenNotice([])).toBeNull();
    expect(withNotTaken('nothing was added — x', [])).toBe('nothing was added — x');
  });
});

// LA-26: files picked with the header's + are one drop, and in the desktop
// app they are handed to it by path exactly as a drop is — a folder picked
// whole going as that folder, which the preload works out — rather than read
// by the page. In a browser they are the page's own files.
describe('files picked with +', () => {
  /** A file of a folder picked whole, by its way down, as the file input hands one over. */
  const picked = (relative: string) => {
    const file = new File(['page'], relative.split('/').at(-1) ?? relative, { lastModified: 7 });
    Object.defineProperty(file, 'webkitRelativePath', { value: relative });
    return file;
  };

  it('are handed to the desktop app, and become the records it answered with', async () => {
    const takeDrop = vi.fn<DesktopBridge['takeDrop']>(async () => BOOK);
    vi.stubGlobal('coffretDesktop', { takeDrop });
    const pages = [picked('vol-1/page-1.jpg'), picked('vol-1/page-2.jpg'), picked('vol-1/.DS_Store')];

    const drop = await pickedFiles(pages);

    expect(takeDrop).toHaveBeenCalledOnce();
    expect(takeDrop).toHaveBeenCalledWith(pages);
    expect(drop.files).toEqual(
      BOOK.files.map((file, index) => ({
        path: file.path,
        size: 100,
        mtime: 1_444_000_000_000,
        source: { kind: 'held', handle: HANDLE, index },
      })),
    );
    // What the system keeps is the app's walk to leave out, and its answer
    // left nothing out here: the page does not read the names itself.
    expect(drop.systemFiles).toEqual([]);
    expect(drop.notTaken).toEqual([]);
  });

  it('say what the desktop app left out, and throw where it took nothing', async () => {
    vi.stubGlobal('coffretDesktop', { takeDrop: async () => PHOTOS_AND_A_LINK });
    const drop = await pickedFiles([new File([''], 'a.jpg')]);
    expect(drop.notTaken).toEqual([LINK]);

    vi.stubGlobal('coffretDesktop', {
      takeDrop: async (): Promise<HeldDrop> => ({ handle: null, files: [], notTaken: [LINK] }),
    });
    await expect(pickedFiles([new File([''], 'link-to-photos')])).rejects.toEqual(new NothingTaken([LINK]));
  });

  it('ask the desktop app nothing where nothing was picked', async () => {
    const takeDrop = vi.fn<DesktopBridge['takeDrop']>(async () => BOOK);
    vi.stubGlobal('coffretDesktop', { takeDrop });

    expect(await pickedFiles([])).toEqual({ files: [], notTaken: [], systemFiles: [] });
    expect(takeDrop).not.toHaveBeenCalled();
  });

  it('are read by the page itself in a browser, with no bridge to hand them to', async () => {
    expect(desktopBridge()).toBeNull();
    const pages = [picked('vol-1/page-1.jpg'), picked('vol-1/.DS_Store')];

    const drop = await pickedFiles(pages);

    expect(drop.files.map((added) => added.path)).toEqual(['vol-1/page-1.jpg']);
    expect(drop.files[0].source).not.toHaveProperty('kind', 'held');
    expect(drop.systemFiles).toEqual([{ path: 'vol-1/.DS_Store', folder: false }]);
    expect(drop.notTaken).toEqual([]);
  });
});

describe('a drop that came to no files', () => {
  it('says what the app did not take, not that the drop could not be read', async () => {
    const bridge: DesktopBridge = {
      takeDrop: async () => ({
        handle: null,
        files: [],
        notTaken: [LINK],
      }),
    };

    const cause = await heldBy(bridge, [new File([''], 'link-to-photos')]).catch(
      (refused: unknown) => refused,
    );

    const line = nothingAddedLine(cause);
    expect(line).toBe(
      'nothing was added — link-to-photos is a link or something else that is neither a file nor a folder',
    );
    expect(line).not.toContain('could not be read');
  });

  it('still says a browser’s walk that was refused could not be read', async () => {
    const refusing: DroppedFolder = {
      isFile: false,
      isDirectory: true,
      name: 'album',
      createReader: () => ({
        readEntries: (_onEntries, onRefused) => onRefused?.(new Error('NotReadableError')),
      }),
    };
    const error = vi.spyOn(console, 'error').mockImplementation(() => undefined);

    const cause = await filesUnder([refusing]).catch((refused: unknown) => refused);

    expect(nothingAddedLine(cause)).toBe(
      'nothing was added — those files could not be read: the explorer could not finish that',
    );
    error.mockRestore();
  });
});

describe('the line saying what was not taken', () => {
  it('calls the rest alike only where they were not taken for the same reason', () => {
    expect(
      notTakenLine([
        { path: 'a', reason: 'reserved' },
        { path: 'b', reason: 'reserved' },
      ]),
    ).toBe('a is a name coffret keeps for itself, and 1 more like it');
    expect(
      notTakenLine([
        { path: 'a', reason: 'reserved' },
        { path: 'b', reason: 'missing' },
        { path: 'c', reason: 'unreadable' },
      ]),
    ).toBe('a is a name coffret keeps for itself, and 2 more not taken');
  });
});

describe('without the bridge', () => {
  it('walks the drop itself and sends its bytes as it always did', async () => {
    expect(desktopBridge()).toBeNull();
    const photo = new File(['a photograph'], 'photo.jpg', { lastModified: 1_444_000_000_999 });

    const drop = await droppedFiles(transfer([photo]));

    expect(drop.notTaken).toEqual([]);
    expect(drop.files).toEqual([
      {
        path: 'photo.jpg',
        size: photo.size,
        mtime: 1_444_000_000_999,
        source: { kind: 'file', file: photo },
      },
    ]);
  });
});
