// A drop in the desktop app's window: the files are handed to the app's bridge
// by path, the records the explorer builds from its answer go through the same
// confirmation and Pack choice a browser's drop does, and they are sent as an
// import naming the handle rather than as bytes.

import { addFiles, dropToken, holdToken, type Added } from '@coffret/api';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  desktopBridge,
  droppedFiles,
  heldBy,
  notTakenLine,
  type DesktopBridge,
  type HeldDrop,
} from './drop';
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

    const added = await droppedFiles(transfer([dropped]));

    expect(takeDrop).toHaveBeenCalledWith([dropped]);
    expect(added).toEqual(
      BOOK.files.map((file, index) => ({
        path: file.path,
        size: 100,
        mtime: 1_444_000_000_000,
        source: { kind: 'held', handle: HANDLE, index },
      })),
    );
  });

  it('goes through the same Pack choice and is sent as an import', async () => {
    vi.stubGlobal('coffretDesktop', { takeDrop: async () => BOOK });
    const made = recording();
    const added = await droppedFiles(transfer([new File(['folder'], 'vol-1')]));
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
    await expect(heldBy(bridge, [])).resolves.toEqual([]);
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

    await expect(heldBy(bridge, [new File(['an image'], 'image.png')])).resolves.toEqual([]);
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

    const added = await droppedFiles(transfer([photo]));

    expect(added).toEqual([
      {
        path: 'photo.jpg',
        size: photo.size,
        mtime: 1_444_000_000_999,
        source: { kind: 'file', file: photo },
      },
    ]);
  });
});
