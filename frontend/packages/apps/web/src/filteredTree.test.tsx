// The explorer's tree filtered from the header's search field: by a name, by
// the most recent books, or by both, drawn from the server's answer of which
// rows a filtered tree keeps — and the tree as it stood coming back once the
// filter is off.

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import {
  getBeside,
  getFile,
  getFilter,
  getFolders,
  getLibrary,
  getLibraryTotals,
  getListing,
  getRemoteHead,
  getWork,
  peekFile,
  readAhead,
  refreshCatalog,
  Refusal,
  type Beside,
  type FilterAsk,
  type Filtered,
  type ListedFile,
  type Listing,
  type Work,
} from '@coffret/api';

import { App } from './App';
import { FILTERING, NO_MATCHES } from './filter';
import { parseHash } from './hash';
import { time } from './humanize';
import { CHECKING } from './refresh';
import { TRY_AGAIN } from './Region';
import { COLOR } from './theme';

vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getAppearance: vi.fn(() => Promise.resolve('system')),
  getBeside: vi.fn(),
  getFile: vi.fn(),
  getFilter: vi.fn(),
  getFolders: vi.fn(),
  getLibrary: vi.fn(),
  getLibraryTotals: vi.fn(),
  getListing: vi.fn(),
  getRemoteHead: vi.fn(),
  getWork: vi.fn(),
  peekFile: vi.fn(),
  readAhead: vi.fn(),
  refreshCatalog: vi.fn(),
}));

const IDLE: Work = {
  server: 'one-process',
  library: 'unlocked',
  catalog: { state: 'caught_up', stopped: null },
  turn: null,
  fill: null,
  sync: null,
  freeze: null,
  delete: null,
  reconnect: null,
  log: [],
  adds: [],
  findings: [],
};

/** When each file of the Library was added, which the Added column says of a file. */
const ADDED = '2026-03-04T05:06:00Z';

function page(path: string): ListedFile {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    size: 10,
    mtime: null,
    deposited_at: ADDED,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
  };
}

/**
 * The Library every case stands in front of, by folder; a name ending in `/`
 * is a folder.
 */
const LIBRARY: Record<string, string[]> = {
  '': ['comics/', 'novels/', 'readme.txt'],
  comics: ['Harbor/', 'Starfall/'],
  'comics/Harbor': ['h1.jpg'],
  'comics/Starfall': ['001.jpg', '002.jpg'],
  novels: ['スターフォール/', 'notes.txt'],
  'novels/スターフォール': ['p2.jpg', '表紙.jpg'],
};

function listing(path: string): Listing {
  const under = (name: string) => (path === '' ? name : `${path}/${name}`);
  const children = LIBRARY[path] ?? [];
  return {
    path,
    mapped: true,
    held: path === '' || path in LIBRARY,
    folders: children
      .filter((name) => name.endsWith('/'))
      .map((name) => ({
        name: name.slice(0, -1),
        path: under(name.slice(0, -1)),
        mapped: true,
        present_throughout: false,
        files_total: 1,
        files_present: 0,
        size: 10,
        mtime: null,
      })),
    files: children.filter((name) => !name.endsWith('/')).map((name) => page(under(name))),
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
}

/** Every file of the Library in reading order: each folder's folders, then its files. */
function readingOrder(folder = ''): string[] {
  const held = listing(folder);
  return [...held.folders.flatMap((one) => readingOrder(one.path)), ...held.files.map((one) => one.path)];
}

/**
 * The files either side of a row in reading order, among those under `under`
 * where it is given — as the server reads them out of its catalog. A folder's
 * place is before everything under it, and the file at its path after it
 * (spec: LA-32).
 */
function besideIn(path: string, folder: boolean, under?: string): Beside {
  const files = readingOrder().filter((one) => under === undefined || one.startsWith(`${under}/`));
  const inside = files.filter((one) => one.startsWith(`${path}/`));
  if (inside.length > 0) {
    const start = files.indexOf(inside[0]);
    const end = start + inside.length;
    return folder
      ? { previous: files[start - 1] ?? null, next: files[start] ?? null }
      : { previous: files[end - 1] ?? null, next: files[end] ?? null };
  }
  const at = files.indexOf(path);
  return at === -1 ? { previous: null, next: null } : { previous: files[at - 1] ?? null, next: files[at + 1] ?? null };
}

function answer(rows: Filtered['rows'], over: Partial<Filtered> = {}): Filtered {
  return { rows, total: rows.length, truncated: false, ...over };
}

/** What a name, or a count of recent books, is answered with in these cases. */
const BY_NAME: Record<string, Filtered> = {
  star: answer([
    { path: 'comics/Starfall', kind: 'folder', deposited_at: '2026-02-01T00:00:00Z' },
    { path: 'novels/スターフォール/表紙.jpg', kind: 'file' },
  ]),
  ｽﾀｰ: answer([{ path: 'novels/スターフォール', kind: 'folder', deposited_at: null }]),
  nothing: answer([]),
};
const RECENT: Filtered = answer([
  { path: 'comics/Harbor', kind: 'folder', deposited_at: '2026-05-06T07:08:00Z' },
  { path: 'comics/Starfall', kind: 'folder', deposited_at: '2026-02-01T00:00:00Z' },
]);

function answered(ask: FilterAsk): Filtered {
  if (ask.name !== undefined && ask.recent !== undefined) {
    return answer(BY_NAME[ask.name]?.rows.filter((row) => row.kind === 'folder') ?? []);
  }
  if (ask.recent !== undefined) {
    return RECENT;
  }
  return BY_NAME[ask.name ?? ''] ?? answer([]);
}

beforeEach(() => {
  window.history.replaceState(null, '', '#');
  Element.prototype.scrollIntoView = () => undefined;
  vi.mocked(getLibrary).mockResolvedValue({ name: 'main', library_id: '1', provider: 's3' });
  vi.mocked(getLibraryTotals).mockResolvedValue({
    objects: 1,
    files_total: 7,
    files_present: 0,
    size: 70,
    mtime: null,
    size_on_device: 0,
    held_parcels: null,
  });
  vi.mocked(getFolders).mockResolvedValue({ folders: Object.keys(LIBRARY).filter((path) => path !== '') });
  vi.mocked(getRemoteHead).mockResolvedValue({ head: 'current' });
  vi.mocked(getWork).mockResolvedValue(IDLE);
  vi.mocked(getListing).mockImplementation((path: string) => Promise.resolve(listing(path)));
  vi.mocked(getFilter).mockImplementation((ask: FilterAsk) => Promise.resolve(answered(ask)));
  vi.mocked(getFile).mockResolvedValue(new Blob(['page']));
  vi.mocked(peekFile).mockResolvedValue(new Blob(['page']));
  vi.mocked(readAhead).mockResolvedValue(new Blob(['page']));
  vi.mocked(getBeside).mockImplementation((path: string, folder: boolean, { under } = {}) =>
    Promise.resolve(besideIn(path, folder, under)),
  );
  URL.createObjectURL = () => 'blob:page';
  URL.revokeObjectURL = () => undefined;
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function tree() {
  return within(screen.getByRole('tree', { name: 'Library' }));
}

/** The rows drawn, top to bottom, each by its key. */
function keys(): string[] {
  return [...screen.getByRole('tree').querySelectorAll('tr[data-key]')].map((row) => row.getAttribute('data-key') ?? '');
}

function row(key: string): HTMLElement {
  const found = screen.getByRole('tree').querySelector(`tr[data-key="${key}"]`);
  if (found === null) {
    throw new Error(`no row ${key}: ${keys().join(', ')}`);
  }
  return found as HTMLElement;
}

/** The colour a row's name is drawn in. */
function inked(key: string): string {
  const name = [...row(key).querySelectorAll('span')].findLast((span) => span.textContent === row(key).title);
  return name?.style.color ?? '';
}

function input(): HTMLInputElement {
  return screen.getByRole('textbox', { name: 'Filter by name' }) as HTMLInputElement;
}

function status(): HTMLElement {
  return screen.getByRole('status', { name: 'filter' });
}

/** The App, with the root's rows drawn. */
async function opened() {
  render(<App />);
  await waitFor(() => expect(keys()).toContain('folder:comics'));
}

/** Types `name` into the field, and waits for the answer to be drawn. */
async function typed(name: string, drawnKey?: string) {
  fireEvent.change(input(), { target: { value: name } });
  await waitFor(() => expect(getFilter).toHaveBeenLastCalledWith({ name }, expect.any(AbortSignal)));
  if (drawnKey !== undefined) {
    await waitFor(() => expect(keys()).toContain(drawnKey));
  }
}

/** Turns Recent on in the panel, with the count it holds. */
function recentOn() {
  fireEvent.click(screen.getByRole('button', { name: 'More filters' }));
  fireEvent.click(within(screen.getByRole('group', { name: 'More filters' })).getByRole('checkbox'));
}

describe('a name', () => {
  it('keeps matched folders closed under their folders drawn dimmed, and a matched file alone', async () => {
    await opened();
    await typed('star', 'folder:comics/Starfall');
    await waitFor(() => expect(keys()).toContain('file:novels/スターフォール/表紙.jpg'));
    expect(keys()).toEqual([
      'folder:comics',
      'folder:comics/Starfall',
      'folder:novels',
      'folder:novels/スターフォール',
      'file:novels/スターフォール/表紙.jpg',
    ]);
    expect(status().textContent).toContain('2 matches');
    // The folders above a match, dimmed and open; the match itself closed.
    expect(inked('folder:comics')).toBe(COLOR.faint);
    expect(inked('folder:novels/スターフォール')).toBe(COLOR.faint);
    expect(inked('folder:comics/Starfall')).toBe(COLOR.text);
    expect(tree().getByLabelText('collapse comics')).toBeTruthy();
    expect(tree().getByLabelText('expand Starfall')).toBeTruthy();
    // No Added column for a name alone.
    expect(tree().queryByRole('columnheader', { name: 'Added' })).toBeNull();

    // A matched folder opened lists everything in it, as without a filter.
    fireEvent.click(tree().getByLabelText('expand Starfall'));
    await waitFor(() => expect(keys()).toContain('file:comics/Starfall/002.jpg'));
    expect(keys()).toContain('file:comics/Starfall/001.jpg');
    // And the rows that did not match are nowhere.
    expect(keys()).not.toContain('folder:comics/Harbor');
    expect(keys()).not.toContain('file:novels/スターフォール/p2.jpg');
    expect(keys()).not.toContain('file:readme.txt');
  });

  it('is sent as typed, full-width and half-width forms matched by the server', async () => {
    await opened();
    await typed('ｽﾀｰ', 'folder:novels/スターフォール');
    expect(keys()).toEqual(['folder:novels', 'folder:novels/スターフォール']);
    expect(status().textContent).toContain('1 match');
  });

  it('that matches nothing says so, with no rows and no empty Library', async () => {
    await opened();
    await typed('nothing');
    await waitFor(() => expect(status().textContent).toContain(NO_MATCHES));
    expect(keys()).toEqual([]);
    expect(document.body.textContent).not.toContain('this Library is empty');
  });

  it('waits until the typing stops before asking', async () => {
    await opened();
    fireEvent.change(input(), { target: { value: 's' } });
    fireEvent.change(input(), { target: { value: 'st' } });
    fireEvent.change(input(), { target: { value: 'sta' } });
    fireEvent.change(input(), { target: { value: 'star' } });
    // The line says it is being asked while the tree stays as it was.
    expect(status().textContent).toContain(FILTERING);
    await waitFor(() => expect(keys()).toContain('folder:comics/Starfall'));
    expect(getFilter).toHaveBeenCalledTimes(1);
  });
});

describe('the most recent books', () => {
  it('are asked for from the panel, with the Added column', async () => {
    await opened();
    recentOn();
    await waitFor(() => expect(getFilter).toHaveBeenLastCalledWith({ recent: 10 }, expect.any(AbortSignal)));
    await waitFor(() => expect(keys()).toContain('folder:comics/Harbor'));
    expect(keys()).toEqual(['folder:comics', 'folder:comics/Harbor', 'folder:comics/Starfall']);
    expect(screen.getByTestId('recent-chip').textContent).toContain('Recent 10');
    expect(tree().getByRole('columnheader', { name: 'Added' })).toBeTruthy();
    // A kept folder's when its book was added; a folder above one blank.
    const cells = (key: string) => [...row(key).querySelectorAll('td')].map((cell) => cell.textContent);
    expect(cells('folder:comics/Harbor')).toContain(time('2026-05-06T07:08:00Z'));
    expect(cells('folder:comics')[4]).toBe('');
    // A file's, from its listing, once the folder is opened; in path order,
    // not sorted by it.
    fireEvent.click(tree().getByLabelText('expand Harbor'));
    await waitFor(() => expect(keys()).toContain('file:comics/Harbor/h1.jpg'));
    expect(cells('file:comics/Harbor/h1.jpg')[4]).toBe(time(ADDED));
  });

  it('with a name: the chip and the name both shown, and both asked', async () => {
    await opened();
    recentOn();
    fireEvent.change(screen.getByRole('spinbutton', { name: 'How many recent folders' }), { target: { value: '2' } });
    fireEvent.change(input(), { target: { value: 'star' } });
    await waitFor(() =>
      expect(getFilter).toHaveBeenLastCalledWith({ name: 'star', recent: 2 }, expect.any(AbortSignal)),
    );
    await waitFor(() => expect(keys()).toEqual(['folder:comics', 'folder:comics/Starfall']));
    expect(screen.getByTestId('recent-chip').textContent).toContain('Recent 2');
    expect(input().value).toBe('star');
    expect(tree().getByRole('columnheader', { name: 'Added' })).toBeTruthy();
    // Only the last of the changes was asked.
    expect(getFilter).toHaveBeenCalledTimes(1);
  });
});

it('says how many it shows of how many where the answer stops short', async () => {
  vi.mocked(getFilter).mockResolvedValue(
    answer([{ path: 'comics/Starfall', kind: 'folder', deposited_at: null }], { total: 8412, truncated: true }),
  );
  await opened();
  await typed('star', 'folder:comics/Starfall');
  expect(status().textContent).toContain('Showing the first 1 of 8,412 matches — narrow the name to see the rest');
});

describe('turning the filter off', () => {
  /** The normal tree with `novels` open and the focus on its `notes.txt`. */
  async function standing() {
    await opened();
    fireEvent.click(tree().getByLabelText('expand novels'));
    await waitFor(() => expect(keys()).toContain('file:novels/notes.txt'));
    fireEvent.click(row('file:novels/notes.txt'));
    await waitFor(() => expect(row('file:novels/notes.txt').getAttribute('aria-selected')).toBe('true'));
  }

  const NORMAL = [
    'folder:comics',
    'folder:novels',
    'folder:novels/スターフォール',
    'file:novels/notes.txt',
    'file:readme.txt',
  ];

  it('by Clear brings the tree back with the folders that were open', async () => {
    await standing();
    await typed('star', 'folder:comics/Starfall');
    fireEvent.click(within(status()).getByRole('button', { name: 'Clear' }));
    await waitFor(() => expect(keys()).toEqual(NORMAL));
    expect(screen.queryByRole('status', { name: 'filter' })).toBeNull();
    expect(input().value).toBe('');
    // Nothing was focused while filtered, so the focus is back where it was.
    expect(row('file:novels/notes.txt').getAttribute('aria-selected')).toBe('true');
  });

  it('by emptying the name, the focus left on the row it was on with its folders opened', async () => {
    await standing();
    await typed('star', 'folder:comics/Starfall');
    fireEvent.click(tree().getByLabelText('expand Starfall'));
    await waitFor(() => expect(keys()).toContain('file:comics/Starfall/002.jpg'));
    fireEvent.click(row('file:comics/Starfall/002.jpg'));
    fireEvent.change(input(), { target: { value: '' } });
    await waitFor(() => expect(keys()).toContain('file:novels/notes.txt'));
    expect(keys()).toEqual([
      'folder:comics',
      'folder:comics/Harbor',
      'folder:comics/Starfall',
      'file:comics/Starfall/001.jpg',
      'file:comics/Starfall/002.jpg',
      ...NORMAL.slice(1),
    ]);
    expect(row('file:comics/Starfall/002.jpg').getAttribute('aria-selected')).toBe('true');
    // Nothing was asked for no filter at all.
    expect(getFilter).toHaveBeenCalledTimes(1);
  });

  it('by the chip’s ×, where Recent was the whole filter', async () => {
    await standing();
    recentOn();
    await waitFor(() => expect(keys()).toContain('folder:comics/Harbor'));
    fireEvent.click(screen.getByRole('button', { name: 'Turn off Recent' }));
    await waitFor(() => expect(keys()).toEqual(NORMAL));
    expect(tree().queryByRole('columnheader', { name: 'Added' })).toBeNull();
  });
});

it('lets go of a request a newer one replaces, and draws only the newer answer', async () => {
  const held: ((filtered: Filtered) => void)[] = [];
  const signals: AbortSignal[] = [];
  vi.mocked(getFilter).mockImplementation(
    (_ask: FilterAsk, signal?: AbortSignal) =>
      new Promise<Filtered>((resolve) => {
        signals.push(signal as AbortSignal);
        held.push(resolve);
      }),
  );
  await opened();
  fireEvent.change(input(), { target: { value: 'nothing' } });
  await waitFor(() => expect(getFilter).toHaveBeenCalledTimes(1));
  fireEvent.change(input(), { target: { value: 'star' } });
  await waitFor(() => expect(getFilter).toHaveBeenCalledTimes(2));
  expect(signals[0].aborted).toBe(true);
  expect(signals[1].aborted).toBe(false);
  await act(async () => {
    held[1](BY_NAME.star);
  });
  await waitFor(() => expect(keys()).toContain('folder:comics/Starfall'));
  // The older answer, arriving late, is not drawn.
  await act(async () => {
    held[0](BY_NAME.nothing);
  });
  expect(keys()).toContain('folder:comics/Starfall');
  expect(status().textContent).toContain('2 matches');
});

it('says a refused request on its line, keeps the rows, and asks again', async () => {
  await opened();
  await typed('star', 'folder:comics/Starfall');
  vi.mocked(getFilter).mockRejectedValueOnce(new Refusal('server', 500, 'the catalog could not be read'));
  fireEvent.change(input(), { target: { value: 'starf' } });
  await waitFor(() => expect(status().textContent).toContain('the catalog could not be read'));
  expect(keys()).toContain('folder:comics/Starfall');
  fireEvent.click(within(status()).getByRole('button', { name: TRY_AGAIN }));
  await waitFor(() => expect(getFilter).toHaveBeenCalledTimes(3));
  await waitFor(() => expect(status().textContent).not.toContain('the catalog could not be read'));
});

it('asks again after something the listings are asked again for', async () => {
  vi.mocked(refreshCatalog).mockResolvedValue({ advanced: true, gained: 1, entries: 8 });
  await opened();
  await typed('star', 'folder:comics/Starfall');
  expect(getFilter).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button', { name: CHECKING }));
  await waitFor(() => expect(getFilter).toHaveBeenCalledTimes(2));
  expect(getFilter).toHaveBeenLastCalledWith({ name: 'star' }, expect.any(AbortSignal));
});

describe('the keys', () => {
  it('Ctrl+F and / put the focus in the field, but not under a dialog', async () => {
    await opened();
    screen.getByRole('tree').focus();
    fireEvent.keyDown(screen.getByRole('tree'), { key: 'f', ctrlKey: true });
    expect(document.activeElement).toBe(input());
    screen.getByRole('tree').focus();
    fireEvent.keyDown(screen.getByRole('tree'), { key: '/' });
    expect(document.activeElement).toBe(input());

    // The New folder dialog, from the header's +.
    fireEvent.click(screen.getByRole('button', { name: 'Add files' }));
    fireEvent.click(await screen.findByRole('menuitem', { name: 'New folder…' }));
    const dialog = await screen.findByRole('dialog');
    const name = within(dialog).getByLabelText('folder name');
    fireEvent.keyDown(name, { key: '/' });
    fireEvent.keyDown(document.body, { key: 'f', ctrlKey: true });
    expect(document.activeElement).not.toBe(input());
  });

  it('↓ and Enter go to the first row, and the two Escapes empty the name and go back to the tree', async () => {
    await opened();
    await typed('star', 'folder:comics/Starfall');
    fireEvent.keyDown(input(), { key: 'ArrowDown' });
    expect(document.activeElement).toBe(screen.getByRole('tree'));
    expect(row('folder:comics').getAttribute('aria-selected')).toBe('true');

    input().focus();
    fireEvent.keyDown(screen.getByRole('tree'), { key: 'ArrowDown' });
    input().focus();
    fireEvent.keyDown(input(), { key: 'Enter' });
    expect(document.activeElement).toBe(screen.getByRole('tree'));
    expect(row('folder:comics').getAttribute('aria-selected')).toBe('true');

    input().focus();
    fireEvent.keyDown(input(), { key: 'Escape' });
    expect(input().value).toBe('');
    expect(document.activeElement).toBe(input());
    fireEvent.keyDown(input(), { key: 'Escape' });
    expect(document.activeElement).toBe(screen.getByRole('tree'));
  });
});

describe('reading on with the reader open', () => {
  /** Opens the file whose row `key` names in the reader, from the tree. */
  async function read(key: string) {
    fireEvent.click(row(key));
    fireEvent.keyDown(screen.getByRole('tree'), { key: 'Enter' });
    await waitFor(() => expect(parseHash(window.location.hash).open).toBe(key.slice('file:'.length)));
    await screen.findByRole('region', { name: 'Reader' });
  }

  /** The file the address says the reader is on. */
  function inReader() {
    return parseHash(window.location.hash).open;
  }

  function pressed(key: 'ArrowDown' | 'ArrowUp') {
    fireEvent.keyDown(screen.getByRole('tree'), { key });
  }

  /**
   * What the server was asked for a key pressed: the reader's read ahead past
   * the end of a folder asks too, with a signal to let go of it by.
   */
  function pressedFor() {
    return vi.mocked(getBeside).mock.calls.filter(([, , options]) => options?.signal === undefined);
  }

  /** Holds the answers to what a key pressed asks, to hand over one by one; the read ahead is answered at once. */
  function holding(): ((beside: Beside) => void)[] {
    const held: ((beside: Beside) => void)[] = [];
    vi.mocked(getBeside).mockImplementation((path, folder, options = {}) =>
      options.signal === undefined
        ? new Promise<Beside>((resolve) => held.push(resolve))
        : Promise.resolve(besideIn(path, folder, options.under)),
    );
    return held;
  }

  /** Recent on, `comics/Harbor` opened and its file in the reader: `comics/Starfall`, kept, closed after it. */
  async function readingHarbor() {
    await opened();
    recentOn();
    await waitFor(() => expect(keys()).toContain('folder:comics/Harbor'));
    fireEvent.click(tree().getByLabelText('expand Harbor'));
    await waitFor(() => expect(keys()).toContain('file:comics/Harbor/h1.jpg'));
    await read('file:comics/Harbor/h1.jpg');
  }

  it('reads past the files a name did not match, onto the next kept file, and stops at the last', async () => {
    await opened();
    await typed('star', 'folder:comics/Starfall');
    fireEvent.click(tree().getByLabelText('collapse novels'));
    fireEvent.click(tree().getByLabelText('expand Starfall'));
    await waitFor(() => expect(keys()).toContain('file:comics/Starfall/002.jpg'));
    await read('file:comics/Starfall/002.jpg');

    // `novels/スターフォール/p2.jpg` comes next in the Library, but only `表紙.jpg` was kept.
    pressed('ArrowDown');
    await waitFor(() => expect(inReader()).toBe('novels/スターフォール/表紙.jpg'));
    expect(tree().getByLabelText('collapse novels')).toBeTruthy();
    // The page knew which file the filter kept, so nothing was asked — nor
    // read ahead from anywhere else.
    expect(getBeside).not.toHaveBeenCalled();

    // The last file inside: `novels/notes.txt` and `readme.txt` are not.
    pressed('ArrowDown');
    await act(async () => undefined);
    expect(inReader()).toBe('novels/スターフォール/表紙.jpg');
  });

  it('asks the server for the first file of the next kept folder, and that folder’s alone', async () => {
    await readingHarbor();
    pressed('ArrowDown');
    await waitFor(() => expect(inReader()).toBe('comics/Starfall/001.jpg'));
    expect(pressedFor()).toEqual([['comics/Starfall', true, { under: 'comics/Starfall' }]]);
    expect(tree().getByLabelText('collapse Starfall')).toBeTruthy();
  });

  it('lets go of an answer that arrives after the key is pressed again', async () => {
    await readingHarbor();
    const held = holding();
    pressed('ArrowDown');
    await waitFor(() => expect(held).toHaveLength(1));
    pressed('ArrowDown');
    await waitFor(() => expect(held).toHaveLength(2));
    await act(async () => {
      held[0]({ previous: null, next: 'comics/Starfall/002.jpg' });
    });
    expect(inReader()).toBe('comics/Harbor/h1.jpg');
    await act(async () => {
      held[1]({ previous: null, next: 'comics/Starfall/001.jpg' });
    });
    await waitFor(() => expect(inReader()).toBe('comics/Starfall/001.jpg'));
  });

  it('lets go of an answer that arrives after the filter is turned off, and reads the whole Library then', async () => {
    await readingHarbor();
    const held = holding();
    pressed('ArrowDown');
    await waitFor(() => expect(held).toHaveLength(1));
    fireEvent.click(screen.getByRole('button', { name: 'Turn off Recent' }));
    await waitFor(() => expect(keys()).toContain('file:readme.txt'));
    await act(async () => {
      held[0]({ previous: null, next: 'comics/Starfall/001.jpg' });
    });
    expect(inReader()).toBe('comics/Harbor/h1.jpg');

    // Off, reading on is the whole Library's, asked as it is without a filter.
    vi.mocked(getBeside).mockImplementation((path, folder, options = {}) => Promise.resolve(besideIn(path, folder, options.under)));
    vi.mocked(getBeside).mockClear();
    pressed('ArrowDown');
    await waitFor(() => expect(inReader()).toBe('comics/Starfall/001.jpg'));
    expect(pressedFor()).toEqual([['comics/Harbor/h1.jpg', false]]);
  });
});
