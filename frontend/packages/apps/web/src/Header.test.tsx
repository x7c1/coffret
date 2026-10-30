import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import type { Library } from '@coffret/api';

import { Header } from './Header';
import { LIBRARY_ROOT } from './location';
import { fileKey, folderKey } from './tree';
import type { Asked } from './useAsked';

afterEach(cleanup);

const LIBRARY: Asked<Library> = {
  status: 'ready',
  value: { name: 'books', library_id: 'abc', provider: 'drive' },
};

function mount(focus: string | null, library: Asked<Library> = LIBRARY) {
  const handlers = {
    onAdd: vi.fn(),
    onCollecting: vi.fn(),
    onUnreadable: vi.fn(),
    onKeysBack: vi.fn(),
  };
  const view = render(<Header library={library} focus={focus} {...handlers} />);
  return { ...handlers, view };
}

function header() {
  return within(screen.getByRole('banner'));
}

// The Library's name stands at the left of the header with nothing before it:
// a padlock there would read as "locked", and an arrow as a menu that is not
// there while the server serves one Library.
it('names the Library first in the header, with no icon before it', () => {
  mount(null);
  const name = header().getByTestId('library');
  expect(name.textContent).toBe('books');
  expect(name.children).toHaveLength(0);
  expect(name.previousElementSibling).toBeNull();
  expect(name.tagName).not.toBe('BUTTON');
  // The one icon in the header is the **+**'s plus, at the other end.
  expect(name.parentElement?.querySelector('svg, img')).toBeNull();
  expect([...screen.getByRole('banner').querySelectorAll('svg, img')]).toEqual([
    screen.getByRole('button', { name: 'Add files' }).querySelector('svg'),
  ]);
});

it('says a refused Library request where the name goes', () => {
  mount(null, { status: 'failed', message: 'the server is not running', locked: false } as Asked<Library>);
  expect(header().getByTestId('library').textContent).toBe('the server is not running');
});

it('says where the focus stands, and follows it', () => {
  const { view, ...handlers } = mount(null);
  const at = (focus: string | null) => view.rerender(<Header library={LIBRARY} focus={focus} {...handlers} />);
  const location = () => header().getByTestId('location');
  expect(location().textContent).toBe(LIBRARY_ROOT);
  at(folderKey('Comics'));
  expect(location().textContent).toBe('Comics');
  at(fileKey('Comics/Series A/Vol. 03/001.jpg'));
  expect(location().textContent).toBe('Comics / Series A / Vol. 03 / 001.jpg');
  // The whole of it in the tooltip, for a location cut short.
  expect(location().getAttribute('title')).toBe('Comics / Series A / Vol. 03 / 001.jpg');
  at(null);
  expect(location().textContent).toBe(LIBRARY_ROOT);
});

// The header's +: a menu of the operating system's two pickers, over the
// focused folder — or the folder of the focused file — read as the menu opens.
it('opens Files… and A folder… from Add files, and adds to the focused folder', async () => {
  const picked = [{ name: 'a.jpg', size: 1, lastModified: 0, webkitRelativePath: '' } as File];
  const pick = async (focus: string | null, which: 'Files…' | 'A folder…') => {
    const added = mount(focus);
    const plus = header().getByRole('button', { name: 'Add files' });
    expect(plus.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(plus);
    expect(plus.getAttribute('aria-expanded')).toBe('true');
    const menu = screen.getByRole('menu');
    expect(within(menu).getAllByRole('menuitem').map((item) => item.textContent)).toEqual(['Files…', 'A folder…']);
    const input = screen.getByTestId(which === 'Files…' ? 'pick-files' : 'pick-folder') as HTMLInputElement;
    const click = vi.spyOn(input, 'click').mockImplementation(() => undefined);
    fireEvent.click(within(menu).getByRole('menuitem', { name: which }));
    expect(click).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('menu')).toBeNull();
    Object.defineProperty(input, 'files', { value: picked, configurable: true });
    fireEvent.change(input);
    await waitFor(() => expect(added.onAdd).toHaveBeenCalledTimes(1));
    expect(added.onCollecting).toHaveBeenCalledTimes(1);
    const [into, drop] = added.onAdd.mock.calls[0];
    const title = plus.getAttribute('title');
    cleanup();
    return { into, title, paths: drop.files.map((one: { path: string }) => one.path) };
  };
  expect(await pick(folderKey('albums/2026'), 'Files…')).toEqual({
    into: 'albums/2026',
    title: 'Add files to albums/2026',
    paths: ['a.jpg'],
  });
  expect(await pick(fileKey('albums/cover.png'), 'A folder…')).toEqual({
    into: 'albums',
    title: 'Add files to albums',
    paths: ['a.jpg'],
  });
  expect(await pick(null, 'Files…')).toEqual({ into: '', title: 'Add files to the Library', paths: ['a.jpg'] });
});

it('hands the keys back to the tree when the menu is let go of by Escape', () => {
  const { onKeysBack } = mount(null);
  fireEvent.click(header().getByRole('button', { name: 'Add files' }));
  fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
  expect(screen.queryByRole('menu')).toBeNull();
  expect(onKeysBack).toHaveBeenCalledTimes(1);
});
