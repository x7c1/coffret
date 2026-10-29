import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  addFiles,
  getFile,
  getFolders,
  getLibrary,
  getListing,
  getRemoteHead,
  getWork,
  peekFile,
  previewDelete,
  startDelete,
  Refusal,
  type Delete,
  type ListedFile,
  type Listing,
  type Work,
} from '@coffret/api';

import { App } from './App';
import { parseHash, toHash } from './hash';
import { size, time } from './humanize';
import { ADD } from './packChoice';
import { FORGET_FOLDER, PROSPECTIVE } from './prospective';

// The server, as the screen reaches it. Everything else the package exports is
// the real thing.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  addFiles: vi.fn(),
  getFile: vi.fn(),
  getFolders: vi.fn(),
  getLibrary: vi.fn(),
  getListing: vi.fn(),
  getRemoteHead: vi.fn(),
  getWork: vi.fn(),
  peekFile: vi.fn(),
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
 * A Library of folders and files by folder: each key a folder, each value its
 * children, a name ending in `/` a folder.
 */
function library(tree: Record<string, string[]>): (path: string) => Listing {
  return (path) => {
    const under = (name: string) => (path === '' ? name : `${path}/${name}`);
    const children = tree[path] ?? [];
    return listing(path, {
      held: path === '' || path in tree,
      folders: children
        .filter((name) => name.endsWith('/'))
        .map((name) => ({ name: name.slice(0, -1), path: under(name.slice(0, -1)), mapped: true, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null })),
      files: children.filter((name) => !name.endsWith('/')).map((name) => page(under(name))),
    });
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
  // The reader's pages: jsdom has no object URLs, and the reader only needs
  // one to put on the image.
  vi.mocked(getFile).mockResolvedValue(new Blob(['page']));
  vi.mocked(peekFile).mockResolvedValue(new Blob(['page']));
  URL.createObjectURL = () => 'blob:page';
  URL.revokeObjectURL = () => undefined;
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

/** The middle of the screen, where the one tree stands once the root is listed. */
function tree() {
  const main = document.querySelector('main');
  if (main === null) {
    throw new Error('the screen draws its middle');
  }
  return within(main as HTMLElement);
}

/** Chooses "New folder…" from a row's menu, names it, and presses Prepare. */
async function newFolder(row: string, name: string) {
  fireEvent.click(await tree().findByLabelText(`actions for ${row}`));
  fireEvent.click(tree().getByRole('menuitem', { name: 'New folder…' }));
  await nameIt(name);
}

/** Presses "New folder…" over the rows, for the folder the focus is in, and names it. */
async function newFolderHere(name: string) {
  fireEvent.click(await tree().findByRole('button', { name: 'New folder…' }));
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
    fireEvent.click(await tree().findByRole('button', { name: 'New folder…' }));

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
    await newFolderHere('comics');
    // The listing asked for to judge the name inside `comics` is held back
    // until after Cancel.
    let answer: (listed: Listing) => void = () => undefined;
    vi.mocked(getListing).mockImplementationOnce(
      () => new Promise<Listing>((resolve) => (answer = resolve)),
    );
    await newFolder('comics', 'Series');
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    await act(async () => {
      answer(listing('comics'));
    });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(tree().queryByTitle('Series')).toBeNull();
  });

  it('prepares a path, and one inside it, and a drop there makes them ordinary folders', async () => {
    let committed = false;
    answers = (path) =>
      committed
        ? library({ '': ['comics/'], comics: ['Series/'], 'comics/Series': ['vol-1.jpg'] })(path)
        : listing(path);
    vi.mocked(addFiles).mockImplementation(() => {
      committed = true;
      return Promise.resolve({ written: ['comics/Series/vol-1.jpg'], refused: [] });
    });
    render(<App />);

    // An empty Library has no row; "New folder…" over the rows prepares one
    // under the root.
    await newFolderHere('comics');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(tree().getByTitle('comics').textContent).toContain('not in Library');
    // Nothing was sent for it.
    expect(addFiles).not.toHaveBeenCalled();

    // A path inside it, from its row, which opens it to show the new row.
    await newFolder('comics', 'Series');
    expect(await tree().findByTitle('Series')).toBeTruthy();
    expect(tree().getAllByTitle(PROSPECTIVE)).toHaveLength(2);

    // A drop on its row is confirmed like any other, with both levels marked new.
    fireEvent.dragEnter(screen.getByRole('tree'), { dataTransfer: { types: ['Files'] } });
    fireEvent.dragOver(tree().getByTitle('Series'));
    fireEvent.drop(screen.getByRole('tree'), carrying('vol-1.jpg'));
    const confirm = await screen.findByRole('dialog');
    expect(confirm.textContent).toContain('comics/Series/');
    expect(confirm.textContent).toContain('comics and Series are new');
    await act(async () => {
      fireEvent.click(within(confirm).getByRole('button', { name: ADD }));
    });
    await waitFor(() => expect(addFiles).toHaveBeenCalledTimes(1));
    expect(vi.mocked(addFiles).mock.calls[0][0]).toBe('comics/Series');

    // Read again, the Library names both, and the marks go.
    await waitFor(() => expect(document.body.textContent).not.toContain('not in Library'));
    fireEvent.click(tree().getByLabelText('expand Series'));
    expect(await tree().findByTitle('vol-1.jpg')).toBeTruthy();
  });

  // EP-9: a top-level folder this device maps with nothing under it is drawn,
  // marked, and takes "New folder…".
  it('draws an empty mapped top-level folder as a place to drop into', async () => {
    answers = (path) =>
      path === ''
        ? listing('', {
            mapped: false,
            folders: [{ name: 'albums', path: 'albums', mapped: false, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null }],
            empty_mapped_prefixes: ['comics'],
          })
        : listing(path);
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
    render(<App />);

    await waitFor(() => expect(tree().getByTitle('comics').textContent).toContain('not in Library'));
    expect(tree().getByTitle('albums').textContent).not.toContain('not in Library');
    // The root is not mapped, and a root of folders keeps the banner back; no
    // path can be prepared right under it.
    expect(screen.queryByText('map the Library root…')).toBeNull();
    expect(tree().queryByRole('button', { name: 'New folder…' })).toBeNull();
    await newFolder('comics', 'Series');
    expect(await tree().findByTitle('Series')).toBeTruthy();
  });
});

// A mistyped prepared path is let go of from its own row, with the paths
// prepared under it, and the server hears nothing of it.
it('forgets a prepared path and the ones under it, and asks nothing', async () => {
  answers = (path) => listing(path);
  render(<App />);
  await newFolderHere('comics');
  await newFolder('comics', 'Series');
  await newFolderHere('books');
  expect(await tree().findByTitle('Series')).toBeTruthy();

  const asked = [
    vi.mocked(getListing).mock.calls.length,
    vi.mocked(getFolders).mock.calls.length,
    vi.mocked(getWork).mock.calls.length,
  ];
  fireEvent.click(tree().getByLabelText('actions for comics'));
  fireEvent.click(tree().getByRole('menuitem', { name: FORGET_FOLDER }));

  expect(tree().queryByTitle('comics')).toBeNull();
  expect(tree().queryByTitle('Series')).toBeNull();
  // The others prepared are left as they were.
  expect(tree().getByTitle('books').textContent).toContain('not in Library');
  expect([
    vi.mocked(getListing).mock.calls.length,
    vi.mocked(getFolders).mock.calls.length,
    vi.mocked(getWork).mock.calls.length,
  ]).toEqual(asked);
  expect(addFiles).not.toHaveBeenCalled();
});

// Forgetting the folder the focus is in, or one above it, leaves the screen in
// the folder the forgotten path was prepared in.
it('moves the screen out of a prepared path forgotten under it', async () => {
  answers = (path) => listing(path);
  render(<App />);
  await newFolderHere('comics');
  await newFolder('comics', 'Series');
  fireEvent.click(await tree().findByTitle('Series'));
  await waitFor(() => expect(shownFolder()).toBe('comics'));

  fireEvent.click(tree().getByLabelText('actions for comics'));
  fireEvent.click(tree().getByRole('menuitem', { name: FORGET_FOLDER }));

  await waitFor(() => expect(shownFolder()).toBe(''));
  expect(tree().queryByTitle('comics')).toBeNull();
});

// Walking the tree moves the URL to the folder the focus is in, without a
// history entry for each row the keys pass; opening a folder adds one, at its
// first page.
it('follows the focus in the URL, and opens a folder at its first page', async () => {
  answers = library({ '': ['comics/'], comics: ['Series/'], 'comics/Series': ['001.jpg', '002.jpg'] });
  vi.mocked(getFolders).mockResolvedValue({ folders: ['comics', 'comics/Series'] });
  render(<App />);
  await tree().findByTitle('comics');
  const area = screen.getByRole('tree');
  const before = window.history.length;

  fireEvent.keyDown(area, { key: 'ArrowDown' });
  fireEvent.keyDown(area, { key: 'ArrowRight' });
  await waitFor(() => expect(shownFolder()).toBe('comics'));
  await tree().findByTitle('Series');
  fireEvent.keyDown(area, { key: 'ArrowDown' });
  // A closed folder's rows are not on show, so the folder it stands in is the
  // one the URL names.
  expect(shownFolder()).toBe('comics');
  expect(window.history.length).toBe(before);

  fireEvent.keyDown(area, { key: 'Enter' });
  await waitFor(() =>
    expect(parseHash(window.location.hash)).toEqual({
      folder: 'comics/Series',
      open: 'comics/Series/001.jpg',
    }),
  );
  expect(window.history.length).toBe(before + 1);
});

// The details pane follows the focused row, says what it is in words that fit
// any file, and opens what `Enter` opens.
it('describes the focused row in the details pane, and opens it from there', async () => {
  const base = library({
    '': ['comics/', 'papers/'],
    comics: ['Series/'],
    'comics/Series': ['001.jpg', '002.jpg'],
    papers: ['manual.pdf'],
  });
  // What the Library says of everything below `comics`: its bytes, and when
  // the newest file anywhere in it was modified.
  const modified = '2026-03-04T05:06:00Z';
  answers = (path) => {
    const one = base(path);
    return {
      ...one,
      folders: one.folders.map((folder) =>
        folder.path === 'comics' ? { ...folder, size: 5_000_000, mtime: modified } : folder,
      ),
    };
  };
  render(<App />);
  await tree().findByTitle('comics');
  const area = screen.getByRole('tree');
  const pane = within(screen.getByRole('complementary', { name: 'Details' }));
  expect(pane.getByText('Nothing selected')).toBeTruthy();

  // A folder holding a folder and no images: its items, and no Open.
  fireEvent.keyDown(area, { key: 'ArrowDown' });
  expect(await pane.findByRole('heading', { name: 'comics' })).toBeTruthy();
  await waitFor(() => expect(definition(pane, 'Items')).toBe('1'));
  expect(definition(pane, 'Kind')).toBe('Folder');
  expect(definition(pane, 'Size')).toBe(size(5_000_000));
  expect(definition(pane, 'Modified')).toBe(time(modified));
  expect(definition(pane, 'On this device')).toBe('No');
  expect(pane.queryByRole('button', { name: 'Open' })).toBeNull();

  // A folder of pages, closed: its cover, peeked and never fetched, and Open.
  fireEvent.keyDown(area, { key: 'ArrowRight' });
  await tree().findByTitle('Series');
  fireEvent.keyDown(area, { key: 'ArrowDown' });
  expect(await pane.findByRole('heading', { name: 'Series' })).toBeTruthy();
  await waitFor(() => expect(peekFile).toHaveBeenCalledWith('comics/Series/001.jpg', expect.anything()));
  expect(getFile).not.toHaveBeenCalled();
  fireEvent.click(await pane.findByRole('button', { name: 'Open' }));
  await waitFor(() =>
    expect(parseHash(window.location.hash)).toEqual({
      folder: 'comics/Series',
      open: 'comics/Series/001.jpg',
    }),
  );
});

// A focused folder whose listing was refused does not count its items as
// still coming.
it('says no item count for a focused folder whose listing was refused', async () => {
  answers = library({ '': ['comics/'], comics: ['001.jpg'] });
  vi.mocked(getListing).mockImplementation((path: string) =>
    path === 'comics' ? Promise.reject(new Error('refused')) : Promise.resolve(answers(path)),
  );
  render(<App />);
  await tree().findByTitle('comics');
  const pane = within(screen.getByRole('complementary', { name: 'Details' }));
  fireEvent.keyDown(screen.getByRole('tree'), { key: 'ArrowDown' });
  expect(await pane.findByRole('heading', { name: 'comics' })).toBeTruthy();
  await waitFor(() => expect(definition(pane, 'Items')).toBe('—'));
  expect(pane.queryByRole('button', { name: 'Open' })).toBeNull();
});

/** What the details pane says against one fact's name. */
function definition(pane: ReturnType<typeof within>, name: string): string | null {
  const term = pane.getByText(name, { selector: 'dt' });
  return term.nextElementSibling?.textContent ?? null;
}

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
 * Deletes the folder `name` from its row of the tree. The Library becomes `after`
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
          ? listing('', {
              folders: [{ name: 'albums', path: 'albums', mapped: true, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null }],
              empty_mapped_prefixes: ['comics'],
            })
          : listing('', {
              folders: [
                { name: 'albums', path: 'albums', mapped: true, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null },
                { name: 'comics', path: 'comics', mapped: true, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null },
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
    await waitFor(() => expect(tree().getByTitle('comics').textContent).toContain('not in Library'));
    expect(shownFolder()).toBe('albums');
  });

  // At the root the listing asked again is the root's own, and nothing else is
  // asked for it.
  it('asks the root once where the screen is on it', async () => {
    answers = (path) =>
      path === ''
        ? listing('', {
            folders: [{ name: 'comics', path: 'comics', mapped: true, present_throughout: false, files_total: 1, files_present: 0, size: 0, mtime: null }],
          })
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
    answers = library({
      '': ['comics/'],
      comics: ['Other/', 'Series/', '1.jpg'],
      'comics/Other': ['1.jpg'],
      'comics/Series': ['1.jpg'],
    });
    vi.mocked(getFolders).mockResolvedValue({
      folders: ['comics', 'comics/Other', 'comics/Series'],
    });
    window.history.replaceState(null, '', toHash({ folder: 'comics/Series', open: null }));
    render(<App />);
    await waitFor(() => expect(tree().getAllByTitle('1.jpg').length).toBeGreaterThan(0));

    await deleteFromTree('Series', 'comics/Series', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: ['comics', 'comics/Other'] });
    });
    await waitFor(() => expect(shownFolder()).toBe('comics'), {
      timeout: 3000,
    });
    expect(noticeSaid()).toContain('removed from the Library');
  });

  it('moves the list to the root where nothing above is held any longer', async () => {
    answers = library({
      '': ['albums/', 'comics/'],
      albums: ['1.jpg'],
      comics: ['Series/'],
      'comics/Series': ['1.jpg'],
    });
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums', 'comics', 'comics/Series'] });
    window.history.replaceState(null, '', toHash({ folder: 'comics/Series', open: null }));
    render(<App />);
    await waitFor(() => expect(tree().getAllByTitle('1.jpg').length).toBeGreaterThan(0));

    await deleteFromTree('Series', 'comics/Series', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
    });
    await waitFor(() => expect(shownFolder()).toBe(''), {
      timeout: 3000,
    });
    expect(noticeSaid()).toContain('removed from the Library');
  });

  it('leaves the screen where it is when the folder deleted is elsewhere', async () => {
    answers = library({ '': ['albums/', 'comics/'], albums: ['1.jpg'], comics: ['1.jpg'] });
    vi.mocked(getFolders).mockResolvedValue({ folders: ['albums', 'comics'] });
    window.history.replaceState(null, '', toHash({ folder: 'albums', open: null }));
    render(<App />);
    await waitFor(() => expect(tree().getAllByTitle('1.jpg').length).toBeGreaterThan(0));

    await deleteFromTree('comics', 'comics', () => {
      vi.mocked(getFolders).mockResolvedValue({ folders: ['albums'] });
      answers = library({ '': ['albums/'], albums: ['1.jpg'] });
    });
    await waitFor(() => expect(noticeSaid()).toContain('removed from the Library'), {
      timeout: 3000,
    });
    await waitFor(() => expect(tree().queryByTitle('comics')).toBeNull());
    expect(shownFolder()).toBe('albums');
  });
});

// The tree draws from the listings alone, so a refusal of the Library's
// folders would otherwise leave nothing on the screen saying it, and nothing
// to press to ask again — while the stranded folders and the way out of a
// deleted folder wait on that answer.
it('says a refusal of the Library’s folders over the tree, and asks again', async () => {
  answers = library({ '': ['comics/'] });
  vi.mocked(getFolders).mockRejectedValue(new Refusal('server', 500, 'the catalog could not be read'));
  render(<App />);
  await tree().findByTitle('comics');
  expect(await tree().findByText('the catalog could not be read')).toBeTruthy();

  vi.mocked(getFolders).mockResolvedValue({ folders: ['comics'] });
  fireEvent.click(tree().getByRole('button', { name: 'try again' }));
  await waitFor(() => expect(tree().queryByText('the catalog could not be read')).toBeNull());
  expect(tree().getByTitle('comics')).toBeTruthy();
});
