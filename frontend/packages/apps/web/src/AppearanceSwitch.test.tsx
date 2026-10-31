import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import type { Appearance } from '@coffret/api';

import { AppearanceSwitch, type KeptAppearance } from './AppearanceSwitch';
import { ShownAppearance } from './ShownAppearance';

afterEach(cleanup);

/** What a server keeps for one device, across every page drawn over it. */
function keptOnTheDevice(start: Appearance = 'system') {
  let held = start;
  const kept: KeptAppearance & { held: () => Appearance } = {
    read: vi.fn(() => Promise.resolve(held)),
    write: vi.fn((appearance: Appearance) => {
      held = appearance;
      return Promise.resolve(held);
    }),
    held: () => held,
  };
  return kept;
}

/** One page: its root, what it shows, and the switch drawn over it. */
function page(kept: KeptAppearance) {
  const root = document.createElement('div');
  // A system that asks for light, so a page drawn dark is one chosen.
  const shown = new ShownAppearance(null, root);
  shown.start();
  const view = render(<AppearanceSwitch onKeysBack={() => {}} shown={shown} kept={kept} />);
  return { root, shown, view };
}

const button = () => screen.getByRole('button', { name: /^Appearance/ });

// The button says what it is and where it stands, in words and in its icon;
// its menu offers the three, the one shown checked, and choosing one draws it
// at once and keeps it for the device.
it('opens a menu of System, Light and Dark, the shown one checked, and keeps the one chosen', async () => {
  const kept = keptOnTheDevice();
  const { root } = page(kept);
  await waitFor(() => expect(kept.read).toHaveBeenCalled());

  expect(button().getAttribute('aria-label')).toBe('Appearance: System');
  expect(button().getAttribute('aria-haspopup')).toBe('menu');
  expect(button().querySelector('svg')?.getAttribute('data-appearance')).toBe('system');

  fireEvent.click(button());
  expect(button().getAttribute('aria-expanded')).toBe('true');
  const menu = screen.getByRole('menu', { name: 'appearance' });
  const items = within(menu).getAllByRole('menuitemradio');
  expect(items.map((item) => [item.textContent, item.getAttribute('aria-checked')])).toEqual([
    ['System', 'true'],
    ['Light', 'false'],
    ['Dark', 'false'],
  ]);

  fireEvent.click(within(menu).getByRole('menuitemradio', { name: 'Dark' }));
  expect(screen.queryByRole('menu')).toBeNull();
  expect(button().getAttribute('aria-label')).toBe('Appearance: Dark');
  expect(button().querySelector('svg')?.getAttribute('data-appearance')).toBe('dark');
  expect(root.dataset.scheme).toBe('dark');
  expect(kept.write).toHaveBeenCalledWith('dark');

  fireEvent.click(button());
  expect(
    within(screen.getByRole('menu')).getByRole('menuitemradio', { name: 'Dark' }).getAttribute('aria-checked'),
  ).toBe('true');
});

// The choice is the device's: a page drawn afresh over it — a reload, or the
// desktop app's next launch — reads it and is drawn in it.
it('draws a page afresh in the choice kept by the one before', async () => {
  const kept = keptOnTheDevice();
  const first = page(kept);
  fireEvent.click(button());
  fireEvent.click(screen.getByRole('menuitemradio', { name: 'Dark' }));
  expect(kept.held()).toBe('dark');
  first.view.unmount();

  const again = page(kept);
  expect(again.root.dataset.scheme).toBe('light');
  await waitFor(() => expect(button().getAttribute('aria-label')).toBe('Appearance: Dark'));
  expect(again.root.dataset.scheme).toBe('dark');
});

// A kept choice that cannot be read leaves the page as it would be had nobody
// chosen; one that cannot be kept stays on this page.
it('stays at System where the kept choice cannot be read, and keeps a choice on the page that cannot be kept', async () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  const kept: KeptAppearance = {
    read: vi.fn(() => Promise.reject(new Error('refused'))),
    write: vi.fn(() => Promise.reject(new Error('refused'))),
  };
  const { root } = page(kept);
  await waitFor(() => expect(warn).toHaveBeenCalledTimes(1));
  expect(button().getAttribute('aria-label')).toBe('Appearance: System');

  fireEvent.click(button());
  fireEvent.click(screen.getByRole('menuitemradio', { name: 'Light' }));
  await waitFor(() => expect(warn).toHaveBeenCalledTimes(2));
  expect(button().getAttribute('aria-label')).toBe('Appearance: Light');
  expect(root.dataset.scheme).toBe('light');
  warn.mockRestore();
});

// A choice made before the kept one arrives is the newer word.
it('lets a choice made on the page stand over a kept one that arrives after it', async () => {
  let answer: (appearance: Appearance) => void = () => {};
  const kept: KeptAppearance = {
    read: () => new Promise((resolve) => (answer = resolve)),
    write: vi.fn((appearance: Appearance) => Promise.resolve(appearance)),
  };
  page(kept);
  fireEvent.click(button());
  fireEvent.click(screen.getByRole('menuitemradio', { name: 'Light' }));
  await act(async () => answer('dark'));
  expect(button().getAttribute('aria-label')).toBe('Appearance: Light');
});

// `Escape` lets the menu go and hands the keys back.
it('hands the keys back when the menu is let go of by Escape', () => {
  const onKeysBack = vi.fn();
  const shown = new ShownAppearance(null, document.createElement('div'));
  render(<AppearanceSwitch onKeysBack={onKeysBack} shown={shown} kept={keptOnTheDevice()} />);
  fireEvent.click(button());
  fireEvent.keyDown(document, { key: 'Escape' });
  expect(screen.queryByRole('menu')).toBeNull();
  expect(onKeysBack).toHaveBeenCalled();
});

// Choosing from the keys leaves them somewhere: back on the tree, as `Escape`
// does, rather than on the item that went with the menu.
it('hands the keys back once a choice is made', () => {
  const onKeysBack = vi.fn();
  const shown = new ShownAppearance(null, document.createElement('div'));
  render(<AppearanceSwitch onKeysBack={onKeysBack} shown={shown} kept={keptOnTheDevice()} />);
  fireEvent.click(button(), { detail: 0 });
  const menu = screen.getByRole('menu', { name: 'appearance' });
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  fireEvent.keyDown(menu, { key: 'ArrowDown' });
  fireEvent.keyDown(menu, { key: 'Enter' });
  expect(screen.queryByRole('menu')).toBeNull();
  expect(shown.appearance).toBe('dark');
  expect(onKeysBack).toHaveBeenCalledTimes(1);
});

// A choice the host refused — the Library was locked — stays on the page, and
// is kept when the unlock draws the switch afresh, rather than lost at the
// next launch; the kept one is then not read over it.
it('keeps a choice the locked Library refused once the switch is drawn afresh', async () => {
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  const device = keptOnTheDevice('light');
  let locked = true;
  const kept: KeptAppearance = {
    read: vi.fn((signal: AbortSignal) => (locked ? Promise.reject(new Error('locked')) : device.read(signal))),
    write: vi.fn((appearance: Appearance) =>
      locked ? Promise.reject(new Error('locked')) : device.write(appearance),
    ),
  };
  const { shown, view } = page(kept);
  fireEvent.click(button());
  fireEvent.click(screen.getByRole('menuitemradio', { name: 'Dark' }));
  await waitFor(() => expect(warn).toHaveBeenCalledTimes(2));
  expect(device.held()).toBe('light');
  view.unmount();

  locked = false;
  render(<AppearanceSwitch onKeysBack={() => {}} shown={shown} kept={kept} />);
  await waitFor(() => expect(device.held()).toBe('dark'));
  expect(kept.read).toHaveBeenCalledTimes(1);
  expect(button().getAttribute('aria-label')).toBe('Appearance: Dark');
  warn.mockRestore();
});
