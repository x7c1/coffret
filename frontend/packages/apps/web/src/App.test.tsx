import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  addFiles,
  getFolders,
  getLibrary,
  getListing,
  getRemoteHead,
  getWork,
  type ListedFile,
  type Listing,
  type Work,
} from '@coffret/api';

import { App } from './App';
import { AWAITING } from './awaiting';
import { ADD } from './packChoice';

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
    expect(document.body.textContent).not.toContain(AWAITING);
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
    expect(row.textContent).toContain(AWAITING);
    expect(tree().getByTitle(AWAITING)).toBeTruthy();
    // Nothing was sent for it.
    expect(addFiles).not.toHaveBeenCalled();

    // A path inside it, from the list's row.
    await newFolder(list(), 'comics', 'Series');
    expect(tree().getByTitle('Series')).toBeTruthy();
    expect(tree().getAllByTitle(AWAITING)).toHaveLength(2);

    // Opened, each is an empty listing a drop can target.
    fireEvent.doubleClick(list().getByTitle('comics'));
    await waitFor(() => expect(list().getByTitle('Series').textContent).toContain(AWAITING));
    fireEvent.doubleClick(list().getByTitle('Series'));
    await waitFor(() => expect(getListing).toHaveBeenCalledWith('comics/Series', expect.anything()));
    expect(await list().findByText(AWAITING)).toBeTruthy();

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
      expect(list().getByTitle('Series').textContent).not.toContain(AWAITING),
    );
    fireEvent.click(tree().getByTitle('Library'));
    await waitFor(() => expect(list().getByTitle('comics').textContent).not.toContain(AWAITING));
    expect(document.body.textContent).not.toContain(AWAITING);
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

    await waitFor(() => expect(list().getByTitle('comics').textContent).toContain(AWAITING));
    expect(list().getByTitle('albums').textContent).not.toContain(AWAITING);
    expect(await tree().findByTitle(AWAITING)).toBeTruthy();
    // The root is not mapped, and a root of folders keeps the banner back.
    expect(screen.queryByText('map the Library root…')).toBeNull();
    expect(tree().queryByLabelText('actions for Library')).toBeNull();
    await newFolder(list(), 'comics', 'Series');
    expect(tree().getByTitle('Series')).toBeTruthy();
  });
});
