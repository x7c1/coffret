import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { getFile, peekFile, Refusal, type ListedFile, type ListedFolder, type Listing } from '@coffret/api';

import { size } from './humanize';
import { ReaderView } from './ReaderView';
import { COVER_NOT_HERE, NO_IMAGES, UNSUPPORTED, type Sheeted } from './sheet';
import { fileKey, folderKey, type FileRow, type FolderRow } from './tree';
import type { Asked } from './useAsked';

// The server, as the reader reaches it: one picture's bytes, read or peeked
// at. Everything else the package exports is the real thing.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getFile: vi.fn(),
  peekFile: vi.fn(),
}));

const fetched = vi.mocked(getFile);
const peeked = vi.mocked(peekFile);

/** The server's answer to a peek at a file this device does not have. */
function notHere(): Promise<Blob> {
  return Promise.reject(new Refusal('no_such_entry', 404, 'this device holds no file at that path'));
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
  peeked.mockReset();
  fetched.mockImplementation((path: string) => Promise.resolve(new Blob([path])));
  peeked.mockImplementation((path: string) => Promise.resolve(new Blob([path])));
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

// Walking past a book is looking at it: its cover is peeked at, which sets
// nothing coming, where a page is read, which is the book being read.
it("peeks at a folder's cover, and reads a page", async () => {
  const { show } = sheet(folderRow('albums'), { status: 'ready', value: ALBUM });
  expect(await screen.findByAltText('cover of albums')).toBeTruthy();
  expect(peeked).toHaveBeenCalledWith('albums/one.jpg');
  expect(fetched).not.toHaveBeenCalled();

  show(fileRow(1));
  expect(await screen.findByAltText('two.jpg')).toBeTruthy();
  expect(fetched).toHaveBeenCalledWith('albums/two.jpg');
  expect(peeked).not.toHaveBeenCalledWith('albums/two.jpg');
});

// A cover not on this device is said to be not here, on the stack, with what
// the folder holds under it — and not read in its place.
it('says a cover not on this device is not here yet', async () => {
  peeked.mockImplementation(notHere);
  sheet(folderRow('albums'), { status: 'ready', value: ALBUM });
  expect(await screen.findByText(COVER_NOT_HERE)).toBeTruthy();
  expect(screen.getByText(`Folder · 3 items · ${size(20)}`)).toBeTruthy();
  expect(screen.queryByRole('button', { name: 'try again' })).toBeNull();
  expect(screen.queryByRole('img')).toBeNull();
  expect(fetched).not.toHaveBeenCalled();
});

// Stepping into the book from a cover that was not here reads its first page:
// the refused peek is no answer to that.
it('reads the first page after its cover was refused as not here', async () => {
  peeked.mockImplementation(notHere);
  const { show } = sheet(folderRow('albums'), { status: 'ready', value: ALBUM });
  expect(await screen.findByText(COVER_NOT_HERE)).toBeTruthy();

  show(fileRow(0));
  expect(await screen.findByAltText('one.jpg')).toBeTruthy();
  expect(fetched).toHaveBeenCalledWith('albums/one.jpg');
  expect(screen.queryByText(COVER_NOT_HERE)).toBeNull();
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
