import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  addFiles,
  getFolders,
  getLibrary,
  getListing,
  getRemoteHead,
  getWork,
  previewDelete,
  startDelete,
  type Delete,
  type ListedFile,
  type Listing,
  type Work,
} from '@coffret/api';

import { App } from './App';
import { NOT_KNOWN_YET, TAKEN } from './dropTarget';
import { parseHash, toHash } from './hash';
import { ADD } from './packChoice';
import { FORGET_FOLDER, PROSPECTIVE } from './prospective';
import { COLOR } from './theme';

// The server, as the screen reaches it. Everything else the package exports is
// the real thing.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  addFiles: vi.fn(),
  getFolders: vi.fn(),
  getLibrary: vi.fn(),
  getListing: vi.fn(),
  getRemoteHead: vi.fn(),
  getWork: vi.fn(),
  previewDelete: vi.fn(),
  startDelete: vi.fn(),
}));

const IDLE: Work = {
  server: 'one-process',
  library: 'unlocked',
  catalog: { state: 'caught_up', stopped: null },
  kept_parcels: null,
  turn: null,
  fill: null,
  sync: null,
  freeze: null,
  delete: null,
  reconnect: null,
};

function listing(path: string, over: Partial<Listing> = {}): Listing {
  return {
    path,
    mapped: true,
    held: path === '',
    folders: [],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
    ...over,
  };
}

function page(path: string): ListedFile {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'present',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
  };
}

/**
 * The Library each case stands in front of, by folder; `committed` turns it
 * into the one a drop's files landed in and a run carried in.
 */
let answers: (path: string) => Listing = (path) => listing(path);

beforeEach(() => {
  window.history.replaceState(null, '', '#');
  Element.prototype.scrollIntoView = () => undefined;
  vi.mocked(getLibrary).mockResolvedValue({ name: 'main', library_id: '1', provider: 's3' });
  vi.mocked(getFolders).mockResolvedValue({ folders: [] });
  vi.mocked(getRemoteHead).mockResolvedValue({ head: 'same' } as never);
  vi.mocked(getWork).mockResolvedValue(IDLE);
  vi.mocked(getListing).mockImplementation((path: string) => Promise.resolve(answers(path)));
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

/** The tree down the left, and the list on the right. */
function tree() {
  return within(screen.getByRole('navigation', { name: 'folders' }));
}
function list() {
  const main = document.querySelector('main');
  if (main === null) {
    throw new Error('the screen draws its list');
  }
  return within(main as HTMLElement);
}

/** Chooses "New folder…" from a row's menu, names it, and presses Prepare. */
async function newFolder(where: ReturnType<typeof tree>, row: string, name: string) {
  fireEvent.click(await where.findByLabelText(`actions for ${row}`));
  fireEvent.click(where.getByRole('menuitem', { name: 'New folder…' }));
  await nameIt(name);
}

/** Types `name` into the open dialog and presses Prepare. */
async function nameIt(name: string) {
  const dialog = await screen.findByRole('dialog', { name: 'new folder' });
  fireEvent.change(within(dialog).getByLabelText('folder name'), { target: { value: name } });
  await act(async () => {
    fireEvent.click(within(dialog).getByRole('button', { name: 'Prepare' }));
  });
}

/** A drop carrying files, as a browser hands it over. */
function carrying(...names: string[]) {
  const files = names.map((name) => ({ name, size: 10, lastModified: 0 }) as File);
  return {
    dataTransfer: {
      types: ['Files'],
      files,
      items: files.map((file) => ({
        webkitGetAsEntry: () => ({
          isFile: true,
          isDirectory: false,
          name: file.name,
          file: (onFile: (file: File) => void) => onFile(file),
        }),
      })),
    },
  };
}

describe('starting a Library from the explorer', () => {
  it('refuses a name the Library would refuse, and prepares nothing', async () => {
    answers = (path) => listing(path, path === '' ? { folders_on_disk: ['scans'] } : {});
    render(<App />);
    await screen.findByRole('navigation', { name: 'folders' });
    fireEvent.click(await tree().findByLabelText('actions for Library'));
    fireEvent.click(tree().getByRole('menuitem', { name: 'New folder…' }));

    for (const [typed, said] of [
      ['', 'a folder needs a name'],
      ['comics/Series', 'cannot hold a “/”'],
      ['.coffret', '.coffret'],
      ['scans', 'there is already something called scans here'],
    ]) {
      await nameIt(typed);
      expect((await screen.findByRole('alert')).textContent).toContain(said);
    }
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.body.textContent).not.toContain(PROSPECTIVE);
  });

  it('prepares nothing when the dialog is let go of while the name is still being judged', async () => {
    render(<App />);
    await screen.findByRole('navigation', { name: 'folders' });
    // The root's own listing is on hand; the one asked for to judge the name
    // inside `comics` is held back until after Cancel.
    let answer: (listed: Listing) => void = () => undefined;
    fireEvent.click(await tree().findByLabelText('actions for Library'));
    fireEvent.click(tree().getByRole('menuitem', { name: 'New folder…' }));
    await nameIt('comics');
    vi.mocked(getListing).mockImplementationOnce(
      () => new Promise<Listing>((resolve) => (answer = resolve)),
    );
    fireEvent.click(await list().findByLabelText('actions for comics'));
    fireEvent.click(list().getByRole('menuitem', { name: 'New folder…' }));
    await nameIt('Series');
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await act(async () => {
      answer(listing('comics'));
    });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(tree().queryByTitle('Series')).toBeNull();
  });

  it('prepares a path, and one inside it, and a drop there makes them ordinary folders', async () => {
    let committed = false;
    answers = (path) => {
      if (!committed) {
        return listing(path);
      }
      switch (path) {
        case '':
          return listing('', { folders: [{ name: 'comics', path: 'comics', mapped: true }] });
        case 'comics':
          return listing('comics', {
            held: true,
            folders: [{ name: 'Series', path: 'comics/Series', mapped: true }],
          });
        default:
          return listing(path, { held: true, files: [page('comics/Series/vol-1.jpg')] });
      }
    };
    vi.mocked(addFiles).mockImplementation(() => {
      committed = true;
      return Promise.resolve({ written: ['comics/Series/vol-1.jpg'], refused: [] });
    });
    render(<App />);
    await screen.findByRole('navigation', { name: 'folders' });

    // The tree's root row is the one row an empty Library has.
    await newFolder(tree(), 'Library', 'comics');
    expect(screen.queryByRole('dialog')).toBeNull();
    const row = list().getByTitle('comics');
    expect(row.textContent).toContain(PROSPECTIVE);
    expect(tree().getByTitle(PROSPECTIVE)).toBeTruthy();
    // Nothing was sent for it.
    expect(addFiles).not.toHaveBeenCalled();

    // A path inside it, from the list's row.
    await newFolder(list(), 'comics', 'Series');
    expect(tree().getByTitle('Series')).toBeTruthy();
    expect(tree().getAllByTitle(PROSPECTIVE)).toHaveLength(2);

    // Opened, each is an empty listing a drop can target.
    fireEvent.doubleClick(list().getByTitle('comics'));
    await waitFor(() => expect(list().getByTitle('Series').textContent).toContain(PROSPECTIVE));
    fireEvent.doubleClick(list().getByTitle('Series'));
    await waitFor(() => expect(getListing).toHaveBeenCalledWith('comics/Series', expect.anything()));
    expect(await list().findByText(PROSPECTIVE)).toBeTruthy();

    // The drop is confirmed like any other, with both levels marked new.
    const target = document.querySelector('main [data-scroll-area]');
    if (target === null) {
      throw new Error('the list takes the drop');
    }
    fireEvent.drop(target, carrying('vol-1.jpg'));
    const confirm = await screen.findByRole('dialog');
    expect(confirm.textContent).toContain('comics/Series/');
    expect(confirm.textContent).toContain('comics and Series are new');
    await act(async () => {
      fireEvent.click(within(confirm).getByRole('button', { name: ADD }));
    });
    await waitFor(() => expect(addFiles).toHaveBeenCalledTimes(1));
    expect(vi.mocked(addFiles).mock.calls[0][0]).toBe('comics/Series');

    // Read again, the Library names both, and the marks go.
    await waitFor(() => expect(list().getByTitle('vol-1.jpg')).toBeTruthy());
    fireEvent.click(tree().getByTitle('comics'));
    await waitFor(() =>
      expect(list().getByTitle('Series').textContent).not.toContain(PROSPECTIVE),
    );
    fireEvent.click(tree().getByTitle('Library'));
    await waitFor(() => expect(list().getByTitle('comics').textContent).not.toContain(PROSPECTIVE));
    expect(document.body.textContent).not.toContain(PROSPECTIVE);
  });

  // EP-9: a top-level folder this device maps with nothing under it is drawn,
  // marked, in the list and the tree, and takes "New folder…".
  it('draws an empty mapped top-level folder as a place to drop into', async () => {
    answers = (path) =>
      path === ''
        ? listing('', {
            mapped: false,
            folders: [{ name: 'albums', path: 'albums', mapped: false }],
            empty_mapped_prefixes: ['comics'],
          })
        : listing(path);
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
    render(<App />);
    await screen.findByRole('navigation', { name: 'folders' });

    await waitFor(() => expect(list().getByTitle('comics').textContent).toContain(PROSPECTIVE));
    expect(list().getByTitle('albums').textContent).not.toContain(PROSPECTIVE);
    expect(await tree().findByTitle(PROSPECTIVE)).toBeTruthy();
    // The root is not mapped, and a root of folders keeps the banner back.
    expect(screen.queryByText('map the Library root…')).toBeNull();
    expect(tree().queryByLabelText('actions for Library')).toBeNull();
    await newFolder(list(), 'comics', 'Series');
    expect(tree().getByTitle('Series')).toBeTruthy();
  });
});

// A mistyped prepared path is let go of from its own row, with the paths
// prepared under it, and the server hears nothing of it.
it('forgets a prepared path and the ones under it, and asks nothing', async () => {
  answers = (path) => listing(path);
  render(<App />);
  await screen.findByRole('navigation', { name: 'folders' });
  await newFolder(tree(), 'Library', 'comics');
  await newFolder(list(), 'comics', 'Series');
  await newFolder(tree(), 'Library', 'books');
  expect(tree().getByTitle('Series')).toBeTruthy();

  const asked = [
    vi.mocked(getListing).mock.calls.length,
    vi.mocked(getFolders).mock.calls.length,
    vi.mocked(getWork).mock.calls.length,
  ];
  fireEvent.click(tree().getByLabelText('actions for comics'));
  fireEvent.click(tree().getByRole('menuitem', { name: FORGET_FOLDER }));

  expect(tree().queryByTitle('comics')).toBeNull();
  expect(tree().queryByTitle('Series')).toBeNull();
  expect(list().queryByTitle('comics')).toBeNull();
  // The others prepared are left as they were.
  expect(tree().getByTitle('books')).toBeTruthy();
  expect(list().getByTitle('books').textContent).toContain(PROSPECTIVE);
  expect([
    vi.mocked(getListing).mock.calls.length,
    vi.mocked(getFolders).mock.calls.length,
    vi.mocked(getWork).mock.calls.length,
  ]).toEqual(asked);
  expect(addFiles).not.toHaveBeenCalled();

  // From the list's row too.
  fireEvent.click(list().getByLabelText('actions for books'));
  fireEvent.click(list().getByRole('menuitem', { name: FORGET_FOLDER }));
  expect(list().queryByTitle('books')).toBeNull();
  expect(tree().queryByTitle('books')).toBeNull();
});

// Forgetting the folder the list is showing, or one above it, from the tree
// leaves the list where the forgotten path was prepared.
it('moves the list out of a prepared path forgotten under it', async () => {
  answers = (path) => listing(path);
  render(<App />);
  await screen.findByRole('navigation', { name: 'folders' });
  await newFolder(tree(), 'Library', 'comics');
  await newFolder(tree(), 'comics', 'Series');
  fireEvent.click(tree().getByTitle('Series'));
  await waitFor(() => expect(shownFolder()).toBe('comics/Series'));

  fireEvent.click(tree().getByLabelText('actions for comics'));
  fireEvent.click(tree().getByRole('menuitem', { name: FORGET_FOLDER }));

  await waitFor(() => expect(shownFolder()).toBe(''));
  expect(tree().queryByTitle('comics')).toBeNull();
});

/** A deletion of `folder` that ended, every file of it removed. */
function deletionOf(folder: string): Delete {
  return {
    run: 1,
    folder,
    paths: [],
    entries: 3,
    bytes: 30,
    removed: 3,
    rebuilt: 0,
    rebuild_read: 0,
    rebuild_written: 0,
    refused: [],
    missing: [],
    findings: [],
    step: null,
    waits: false,
    waiting: 0,
    status: 'done',
    stopped: null,
  };
}

/**
 * Deletes the book `name` from its row of the tree. The Library becomes `after`
 * — the tree's answer and the listings — once the deletion is armed, and the
 * next work answer says it ended.
 */
async function deleteFromTree(name: string, folder: string, after: () => void) {
  vi.mocked(previewDelete).mockResolvedValue({
    folder,
    paths: [],
    entries: 3,
    bytes: 30,
    removed: 3,
    rebuilt: 0,
    rebuild_read: 0,
    rebuild_written: 0,
    refused: [],
    missing: [],
    after_current: false,
  });
  vi.mocked(startDelete).mockImplementation(() => {
    after();
    vi.mocked(getWork).mockResolvedValue({ ...IDLE, delete: deletionOf(folder) });
    return Promise.resolve(IDLE);
  });
  fireEvent.click(await tree().findByLabelText(`actions for ${name}`));
  fireEvent.click(tree().getByRole('menuitem', { name: 'Delete…' }));
  const asked = await screen.findByRole('dialog');
  await act(async () => {
    fireEvent.click(within(asked).getByRole('button', { name: 'Delete' }));
  });
}

/** The notice over the list, and the empty string where none stands. */
function noticeSaid(): string {
  return [...document.querySelectorAll('main > p')].map((line) => line.textContent).join(' ');
}

/** The folder the screen is on, as the URL says it. */
function shownFolder(): string {
  return parseHash(window.location.hash).folder;
}

/** How many times the listing of the Library root was asked for. */
function rootAsked(): number {
  return vi.mocked(getListing).mock.calls.filter(([path]) => path === '').length;
}

describe('the end of a deletion', () => {
  // A mapped top-level folder whose last Entry left is an empty mapped prefix
  // now, which only the root's listing names: it is asked again, so the tree
  // goes on drawing the folder, and the screen stays where it is.
  it('asks the root again where the screen is elsewhere, and keeps the folder drawn', async () => {
    let deleted = false;
    answers = (path) =>
      path === ''
        ? deleted
          ? listing('', { folders: [{ name: 'albums', path: 'albums', mapped: true }], empty_mapped_prefixes: ['comics'] })
          : listing('', {
              folders: [
                { name: 'albums', path: 'albums', mapped: true },
                { name: 'comics', path: 'comics', mapped: true },
              ],
            })
        : listing(path, { held: true, files: [page(`${path}/1.jpg`)] });
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums', 'comics'] });
    window.history.replaceState(null, '', toHash({ folder: 'albums', open: null }));
    render(<App />);
    await waitFor(() => expect(tree().getByTitle('comics')).toBeTruthy());
    await waitFor(() => expect(rootAsked()).toBe(1));

    await deleteFromTree('comics', 'comics', () => {
      deleted = true;
      vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
    });

    await waitFor(() => expect(noticeSaid()).toContain('removed from the Library'), {
      timeout: 3000,
    });
    await waitFor(() => expect(rootAsked()).toBe(2));
    await waitFor(() =>
      expect(tree().getByTitle('comics').parentElement?.textContent).toContain('not in Library'),
    );
    expect(shownFolder()).toBe('albums');
  });

  // At the root the listing asked again is the root's own, and nothing else is
  // asked for it.
  it('asks the root once where the screen is on it', async () => {
    answers = (path) =>
      path === ''
        ? listing('', { folders: [{ name: 'comics', path: 'comics', mapped: true }] })
        : listing(path, { held: true });
    vi.mocked(getFolders).mockResolvedValue({ folders: ['comics'] });
    render(<App />);
    await waitFor(() => expect(tree().getByTitle('comics')).toBeTruthy());
    expect(rootAsked()).toBe(1);

    await deleteFromTree('comics', 'comics', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: [] });
    });
    await waitFor(() => expect(noticeSaid()).toContain('removed from the Library'), {
      timeout: 3000,
    });
    // The list's own listing, asked again for the deletion, and nothing more.
    await waitFor(() => expect(rootAsked()).toBe(2));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });
    expect(rootAsked()).toBe(2);
  });

  // The folder on the screen gone, the list moves to the nearest folder above
  // that the Library still holds, and the notice saying so stays.
  it('moves the list up from a folder the deletion took away', async () => {
    answers = (path) => listing(path, { held: true, files: [page(`${path}/1.jpg`)] });
    vi.mocked(getFolders).mockResolvedValue({
      folders: ['comics', 'comics/Other', 'comics/Series'],
    });
    window.history.replaceState(null, '', toHash({ folder: 'comics/Series', open: null }));
    render(<App />);
    await waitFor(() => expect(list().getByTitle('1.jpg')).toBeTruthy());

    await deleteFromTree('Series', 'comics/Series', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: ['comics', 'comics/Other'] });
    });
    await waitFor(() => expect(shownFolder()).toBe('comics'), {
      timeout: 3000,
    });
    expect(noticeSaid()).toContain('removed from the Library');
  });

  it('moves the list to the root where nothing above is held any longer', async () => {
    answers = (path) => listing(path, { held: true, files: [page(`${path}/1.jpg`)] });
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums', 'comics', 'comics/Series'] });
    window.history.replaceState(null, '', toHash({ folder: 'comics/Series', open: null }));
    render(<App />);
    await waitFor(() => expect(list().getByTitle('1.jpg')).toBeTruthy());

    await deleteFromTree('Series', 'comics/Series', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
    });
    await waitFor(() => expect(shownFolder()).toBe(''), {
      timeout: 3000,
    });
    expect(noticeSaid()).toContain('removed from the Library');
  });

  it('leaves the screen where it is when the folder deleted is elsewhere', async () => {
    answers = (path) => listing(path, { held: true, files: [page(`${path}/1.jpg`)] });
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums', 'comics'] });
    window.history.replaceState(null, '', toHash({ folder: 'albums', open: null }));
    render(<App />);
    await waitFor(() => expect(list().getByTitle('1.jpg')).toBeTruthy());

    await deleteFromTree('comics', 'comics', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
    });
    await waitFor(() => expect(noticeSaid()).toContain('removed from the Library'), {
      timeout: 3000,
    });
    await waitFor(() => expect(tree().queryByTitle('comics')).toBeNull());
    expect(shownFolder()).toBe('albums');
  });
});

// A row of the tree is read off the root's listing: until it answers, a drag
// over a row is drawn in neither colour and says the answer is not in yet,
// and once it has, each row says what it comes to.
it('keeps a tree row neutral until the root’s listing answers', async () => {
  let answerRoot: (root: Listing) => void = () => undefined;
  vi.mocked(getListing).mockImplementation((path: string) =>
    path === ''
      ? new Promise<Listing>((resolve) => (answerRoot = resolve))
      : Promise.resolve(listing(path, { held: true })),
  );
  vi.mocked(getFolders).mockResolvedValue({ folders: ['albums', 'books'] });
  window.history.replaceState(null, '', toHash({ folder: 'albums', open: null }));
  render(<App />);
  await waitFor(() => expect(tree().getByTitle('books')).toBeTruthy());

  /** The element a row's drag handlers are on, which holds its name. */
  const rowOf = (name: string) => tree().getByTitle(name).parentElement?.parentElement as HTMLElement;
  const carryingFiles = { dataTransfer: { types: ['Files'] } };
  fireEvent.dragEnter(rowOf('albums'), carryingFiles);
  fireEvent.dragEnter(rowOf('books'), carryingFiles);
  for (const name of ['albums', 'books']) {
    expect(rowOf(name).textContent).toContain(NOT_KNOWN_YET);
    expect(rowOf(name).style.outline).toBe(`2px dashed ${COLOR.dim}`);
  }

  await act(async () => {
    answerRoot(
      listing('', {
        mapped: false,
        folders: [
          { name: 'albums', path: 'albums', mapped: true },
          { name: 'books', path: 'books', mapped: false },
        ],
      }),
    );
  });
  expect(rowOf('albums').textContent).toContain(TAKEN);
  expect(rowOf('albums').style.outline).toBe(`2px dashed ${COLOR.added}`);
  expect(rowOf('books').textContent).toContain('a drop here is not taken');
  expect(rowOf('books').style.outline).toBe(`2px dashed ${COLOR.refused}`);
});
