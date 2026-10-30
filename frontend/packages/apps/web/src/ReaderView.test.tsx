import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import {
  getBeside,
  getFile,
  getListing,
  readAhead,
  Refusal,
  type ListedFile,
  type ListedFolder,
  type Listing,
} from '@coffret/api';

import { size } from './humanize';
import { ReaderView } from './ReaderView';
import { NO_IMAGES, UNSUPPORTED, type Sheeted } from './sheet';
import { fileKey, folderKey, type FileRow, type FolderRow } from './tree';
import type { Asked } from './useAsked';

// The server, as the reader reaches it: one picture's bytes, read or read
// ahead, and what it reads on into past the end of a folder. Everything else
// the package exports is the real thing.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getBeside: vi.fn(),
  getFile: vi.fn(),
  getListing: vi.fn(),
  readAhead: vi.fn(),
}));

const fetched = vi.mocked(getFile);
const early = vi.mocked(readAhead);
const beside = vi.mocked(getBeside);
const listed = vi.mocked(getListing);

/** Storage refusing a fetch, as the server says it. */
function storageGone(): Promise<Blob> {
  return Promise.reject(new Refusal('storage', 503, 'Storage did not answer'));
}

function file(path: string, openable = true): ListedFile {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'present',
    container: 'one-file',
    reserved: false,
    openable,
    content_type: openable ? 'image/jpeg' : 'text/plain',
  };
}

const ALBUM: Listing = {
  path: 'albums',
  mapped: true,
  held: true,
  folders: [],
  files: [file('albums/one.jpg'), file('albums/two.jpg'), file('albums/notes.txt', false)],
  folders_on_disk: [],
  empty_mapped_prefixes: [],
};

function fileRow(at: number): FileRow {
  const one = ALBUM.files[at];
  return { kind: 'file', key: fileKey(one.path), path: one.path, name: one.name, depth: 1, file: one, in: ALBUM };
}

function folderRow(path: string): FolderRow {
  const listed: ListedFolder = {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    mapped: true,
    present_throughout: false,
    files_total: 2,
    files_present: 2,
    size: 20,
    mtime: null,
  };
  return {
    kind: 'folder',
    key: folderKey(path),
    path,
    name: listed.name,
    depth: 0,
    folder: listed,
    in: { ...ALBUM, path: '', files: [] },
    expanded: false,
    expandable: true,
    prospective: false,
    stranded: false,
  };
}

beforeEach(() => {
  fetched.mockReset();
  early.mockReset();
  beside.mockReset();
  listed.mockReset();
  fetched.mockImplementation((path: string) => Promise.resolve(new Blob([path])));
  early.mockImplementation((path: string) => Promise.resolve(new Blob([path])));
  // Nothing after the folder, unless a case says otherwise.
  beside.mockResolvedValue({ previous: null, next: null });
  // jsdom has no object URLs, and the reader only needs one to put on the
  // image. Each test file runs in an environment of its own, so these do not
  // reach any other.
  let made = 0;
  URL.createObjectURL = () => `blob:page-${++made}`;
  URL.revokeObjectURL = () => undefined;
});

afterEach(cleanup);

function sheet(row: Sheeted, asked?: Asked<Listing>, escapable = true) {
  const onClose = vi.fn();
  const props = {
    asked,
    counter: { at: 1, of: 3 },
    discarded: 0,
    escapable,
    onClose,
    onFetching: () => undefined,
    onFetched: () => undefined,
    reconnect: null,
    log: [],
  };
  const drawn = render(<ReaderView row={row} {...props} />);
  return { ...drawn, onClose, show: (next: Sheeted) => drawn.rerender(<ReaderView row={next} {...props} />) };
}

it('shows an image as itself, under its folder, its name and where it stands', async () => {
  sheet(fileRow(0));
  expect(await screen.findByAltText('one.jpg')).toBeTruthy();
  const reader = within(screen.getByRole('region', { name: 'Reader' }));
  expect(reader.getByText('albums')).toBeTruthy();
  expect(reader.getByRole('heading', { name: 'one.jpg' })).toBeTruthy();
  expect(reader.getByLabelText('position').textContent).toBe('1 / 3');
});

// A cover on a stack, and what the folder holds: never what its first page
// looks like in the sheet.
it('shows a folder holding images by its cover, and what it holds', async () => {
  sheet(folderRow('albums'), { status: 'ready', value: ALBUM });
  expect(await screen.findByAltText('cover of albums')).toBeTruthy();
  expect(screen.queryByAltText('one.jpg')).toBeNull();
  expect(screen.getByText(`Folder · 3 items · ${size(20)}`)).toBeTruthy();
});

// LA-33: walking past a book is looking at it: its cover is read ahead, which
// sets none of it coming, where a page is read, which is the book being read
// — the page the cover was included, once the focus steps onto it.
it("reads a folder's cover ahead, and reads a page", async () => {
  const { show } = sheet(folderRow('albums'), { status: 'ready', value: ALBUM });
  expect(await screen.findByAltText('cover of albums')).toBeTruthy();
  expect(early).toHaveBeenCalledWith('albums/one.jpg');
  expect(fetched).not.toHaveBeenCalled();
  expect(beside).not.toHaveBeenCalled();

  show(fileRow(0));
  expect(await screen.findByAltText('one.jpg')).toBeTruthy();
  expect(fetched).toHaveBeenCalledWith('albums/one.jpg');
  expect(early).toHaveBeenCalledTimes(1);
});

// A cover this device does not have is fetched like a page, and the wait says
// so, on the stack where it is drawn.
it('fetches a cover this device does not have, and says it is being brought over', async () => {
  early.mockImplementation(() => new Promise<Blob>(() => undefined));
  const away: Listing = { ...ALBUM, files: ALBUM.files.map((one) => ({ ...one, state: 'remote' })) };
  sheet(folderRow('albums'), { status: 'ready', value: away });
  expect(await screen.findByText('this device does not have it yet, so it is being brought over')).toBeTruthy();
  expect(screen.getByText(`Folder · 3 items · ${size(20)}`)).toBeTruthy();
  expect(fetched).not.toHaveBeenCalled();
});

// Stepping into the book from a cover whose read ahead was refused reads its
// first page: the refusal is no answer to that.
it('reads the first page after its cover was refused', async () => {
  early.mockImplementation(storageGone);
  const { show } = sheet(folderRow('albums'), { status: 'ready', value: ALBUM });
  expect(await screen.findByText('Storage did not answer')).toBeTruthy();

  show(fileRow(0));
  expect(await screen.findByAltText('one.jpg')).toBeTruthy();
  expect(fetched).toHaveBeenCalledWith('albums/one.jpg');
});

// LA-33: within ten files of the end of its folder the reader asks
// which file comes after the folder's last, lists that file's folder and reads
// its first pages ahead — none of them read, so the next folder is not set
// coming and the fill of this one goes on.
it('reads ahead into the next folder near the end of this one', async () => {
  const next: Listing = {
    ...ALBUM,
    path: 'albums2',
    files: ['a.jpg', 'b.jpg', 'c.jpg', 'd.jpg'].map((name) => file(`albums2/${name}`)),
  };
  beside.mockResolvedValue({ previous: 'albums/two.jpg', next: 'albums2/a.jpg' });
  listed.mockResolvedValue(next);
  sheet(fileRow(1));
  expect(await screen.findByAltText('two.jpg')).toBeTruthy();

  await waitFor(() => expect(early).toHaveBeenCalledTimes(3));
  expect(beside).toHaveBeenCalledWith('albums/notes.txt', false, expect.anything());
  expect(listed).toHaveBeenCalledWith('albums2', expect.anything());
  expect(early.mock.calls.map(([path]) => path)).toEqual(['albums2/a.jpg', 'albums2/b.jpg', 'albums2/c.jpg']);
  expect(fetched).not.toHaveBeenCalledWith('albums2/a.jpg');
});

// And nothing past the end of the Library: there is no folder to read into.
it('reads nothing ahead at the end of the Library', async () => {
  sheet(fileRow(1));
  expect(await screen.findByAltText('two.jpg')).toBeTruthy();
  await waitFor(() => expect(beside).toHaveBeenCalled());
  expect(listed).not.toHaveBeenCalled();
  expect(early).not.toHaveBeenCalled();
});

it('shows a folder holding no images as the stack saying so', () => {
  sheet(folderRow('papers'), { status: 'ready', value: { ...ALBUM, path: 'papers', files: [file('papers/a.txt', false)] } });
  expect(screen.getByText(NO_IMAGES)).toBeTruthy();
  expect(screen.queryByRole('img')).toBeNull();
  expect(fetched).not.toHaveBeenCalled();
});

it('says a file a browser draws nothing from is not one it can preview', () => {
  sheet(fileRow(2));
  expect(screen.getByText(UNSUPPORTED)).toBeTruthy();
  expect(screen.getAllByText('notes.txt').length).toBeGreaterThan(0);
  expect(fetched).not.toHaveBeenCalled();
});

// The sheet follows the row it is handed, which is the tree's focus.
it('shows the next row it is handed, and keeps the pages around it ready', async () => {
  const { show } = sheet(fileRow(0));
  await screen.findByAltText('one.jpg');
  // The page beside it was prefetched.
  expect(fetched).toHaveBeenCalledWith('albums/two.jpg');
  show(fileRow(1));
  expect(await screen.findByAltText('two.jpg')).toBeTruthy();
  expect(screen.queryByAltText('one.jpg')).toBeNull();
});

// The arrows are the tree's: a book bound right to left pages the other way
// from one bound left to right, so the sheet turns nothing by them.
it('binds no paging to the left and right arrows', async () => {
  const { onClose } = sheet(fileRow(0));
  await screen.findByAltText('one.jpg');
  const calls = fetched.mock.calls.length;
  for (const key of ['ArrowRight', 'ArrowLeft']) {
    const event = new KeyboardEvent('keydown', { key, cancelable: true, bubbles: true });
    window.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  }
  expect(screen.getByAltText('one.jpg')).toBeTruthy();
  expect(screen.getByLabelText('position').textContent).toBe('1 / 3');
  expect(fetched.mock.calls.length).toBe(calls);
  expect(onClose).not.toHaveBeenCalled();
});

// The page is where the pointer rests while somebody reads, and a press there
// is not a request to leave.
it('stays open on a click on it', async () => {
  const { onClose } = sheet(fileRow(0));
  fireEvent.click(await screen.findByAltText('one.jpg'));
  fireEvent.click(screen.getByRole('region', { name: 'Reader' }));
  expect(onClose).not.toHaveBeenCalled();
});

it('closes on its close button', async () => {
  const { onClose } = sheet(fileRow(0));
  fireEvent.click(screen.getByRole('button', { name: 'Close' }));
  expect(onClose).toHaveBeenCalledTimes(1);
});

it('closes on Escape, but not on one a dialog over it or a menu took', async () => {
  const { onClose, unmount } = sheet(fileRow(0));
  const taken = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true });
  taken.preventDefault();
  window.dispatchEvent(taken);
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onClose).toHaveBeenCalledTimes(1);
  unmount();

  const under = sheet(fileRow(0), undefined, false);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(under.onClose).not.toHaveBeenCalled();
});

// The tree's keys move what the sheet shows, so the sheet leaves the
// keyboard's focus with the tree.
it('leaves the focus where it was', async () => {
  const tree = document.createElement('div');
  tree.tabIndex = 0;
  document.body.append(tree);
  tree.focus();
  sheet(fileRow(0));
  await screen.findByAltText('one.jpg');
  expect(document.activeElement).toBe(tree);
  tree.remove();
});
