import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, expect, it, vi } from 'vitest';

import { NO_FILTER, RECENT_EXPLAINED, type FilterSettings } from './filter';
import { FilterField } from './FilterField';

afterEach(cleanup);

/** The field over settings of its own, as the screen holds them. */
function mount(start: FilterSettings = NO_FILTER) {
  const onToTree = vi.fn();
  const onKeysBack = vi.fn();
  const seen: FilterSettings[] = [];
  function Held() {
    const [settings, setSettings] = useState(start);
    return (
      <>
        <input aria-label="another field" />
        <button type="button">elsewhere</button>
        <FilterField
          settings={settings}
          onChange={(next) => {
            seen.push(next);
            setSettings(next);
          }}
          onToTree={onToTree}
          onKeysBack={onKeysBack}
        />
      </>
    );
  }
  render(<Held />);
  return { onToTree, onKeysBack, seen, latest: () => seen[seen.length - 1] };
}

function field() {
  return within(screen.getByRole('search'));
}

function input() {
  return screen.getByRole('textbox', { name: 'Filter by name' });
}

it('holds a magnifier, the name, its ×, a divider and More filters, in that order', () => {
  mount({ ...NO_FILTER, name: 'star' });
  const search = screen.getByRole('search');
  const parts = [...search.children].map((part) =>
    part.tagName === 'svg'
      ? 'magnifier'
      : part.tagName === 'INPUT'
        ? 'input'
        : part.tagName === 'BUTTON'
          ? part.getAttribute('aria-label')
          : 'divider',
  );
  expect(parts).toEqual(['magnifier', 'input', 'Clear the name', 'divider', 'More filters']);
  expect(input().getAttribute('placeholder')).toBe('Filter by name');
});

it('offers the × only while there is a name, and empties it', () => {
  const { latest } = mount();
  expect(field().queryByRole('button', { name: 'Clear the name' })).toBeNull();
  fireEvent.change(input(), { target: { value: 'star' } });
  expect(latest().name).toBe('star');
  fireEvent.click(field().getByRole('button', { name: 'Clear the name' }));
  expect(latest().name).toBe('');
  expect(field().queryByRole('button', { name: 'Clear the name' })).toBeNull();
});

it('turns Recent on in its panel, shows the chip, and turns it off from the chip', () => {
  const { latest } = mount();
  const more = field().getByRole('button', { name: 'More filters' });
  fireEvent.click(more);
  expect(more.getAttribute('aria-expanded')).toBe('true');
  const panel = screen.getByRole('group', { name: 'More filters' });
  const count = within(panel).getByRole('spinbutton', { name: 'How many recent folders' });
  expect((count as HTMLInputElement).value).toBe('10');
  fireEvent.click(within(panel).getByRole('checkbox'));
  expect(latest()).toEqual({ name: '', recentOn: true, recent: 10 });
  expect(screen.getByTestId('recent-chip').textContent).toContain('Recent 10');

  // A count that is not a whole number from 1 changes nothing, and is put back
  // as the field is left.
  fireEvent.change(count, { target: { value: '0' } });
  expect(latest().recent).toBe(10);
  fireEvent.blur(count);
  expect((count as HTMLInputElement).value).toBe('10');
  fireEvent.change(count, { target: { value: '3' } });
  expect(latest().recent).toBe(3);
  expect(screen.getByTestId('recent-chip').textContent).toContain('Recent 3');

  // The chip comes before the name.
  const search = screen.getByRole('search');
  expect([...search.children].indexOf(screen.getByTestId('recent-chip'))).toBeLessThan(
    [...search.children].indexOf(input()),
  );

  fireEvent.click(field().getByRole('button', { name: 'Turn off Recent' }));
  expect(latest()).toEqual({ name: '', recentOn: false, recent: 3 });
  expect(screen.queryByTestId('recent-chip')).toBeNull();
});

it('closes the panel by Escape or a press outside, keeping what was set', () => {
  const { latest } = mount();
  const more = field().getByRole('button', { name: 'More filters' });
  fireEvent.click(more);
  fireEvent.click(within(screen.getByRole('group', { name: 'More filters' })).getByRole('checkbox'));
  fireEvent.keyDown(screen.getByRole('checkbox'), { key: 'Escape' });
  expect(screen.queryByRole('group', { name: 'More filters' })).toBeNull();
  expect(document.activeElement).toBe(more);
  expect(latest().recentOn).toBe(true);

  // From ⏷ itself, where the press that opened the panel left the focus.
  fireEvent.click(more);
  fireEvent.keyDown(more, { key: 'Escape' });
  expect(screen.queryByRole('group', { name: 'More filters' })).toBeNull();

  fireEvent.click(more);
  fireEvent.mouseDown(input());
  expect(screen.queryByRole('group', { name: 'More filters' })).not.toBeNull();
  fireEvent.mouseDown(screen.getByRole('button', { name: 'elsewhere' }));
  expect(screen.queryByRole('group', { name: 'More filters' })).toBeNull();
  expect(latest().recentOn).toBe(true);
});

/** The More filters panel opened, and its question mark ("About Recent"). */
function openedHelp() {
  fireEvent.click(field().getByRole('button', { name: 'More filters' }));
  const panel = screen.getByRole('group', { name: 'More filters' });
  return { panel, help: within(panel).getByRole('button', { name: 'About Recent' }) };
}

/** The explainer, where it is shown: hidden, it is not in the accessibility tree. */
function explainer() {
  return screen.queryByRole('tooltip');
}

it('holds the Recent line and its question mark, and keeps the explainer out of sight until asked for', () => {
  mount();
  const { panel, help } = openedHelp();
  // `Recent [n] folders` and the question mark, and nothing else shown.
  const shown = [...panel.querySelectorAll('label, button, [role="tooltip"]')].filter(
    (part) => !(part as HTMLElement).hidden,
  );
  expect(shown.map((part) => part.textContent)).toEqual(['Recentfolders', '']);
  expect(explainer()).toBeNull();
  // Hidden, it still describes the question mark, word for word.
  const described = document.getElementById(help.getAttribute('aria-describedby') ?? '');
  expect(described?.textContent).toBe(RECENT_EXPLAINED);
  expect(described?.hidden).toBe(true);
});

it('draws the question mark as an icon, with no [?] text', () => {
  mount();
  const { help } = openedHelp();
  expect(help.querySelector('svg')).not.toBeNull();
  expect(help.textContent).toBe('');
  expect(document.getElementById(help.getAttribute('aria-describedby') ?? '')?.textContent).toBe(
    RECENT_EXPLAINED,
  );
});

it('ends the Recent line with the question mark, after the count and "folders"', () => {
  mount();
  const { help } = openedHelp();
  const line = help.parentElement as HTMLElement;
  // What the line shows, in order: the condition, then the question mark last;
  // the explainer beside them is hung under the line, not on it.
  const onLine = [...line.children].filter((part) => part.getAttribute('role') !== 'tooltip');
  expect(onLine.at(-1)).toBe(help);
  const condition = onLine[0] as HTMLElement;
  expect(condition.textContent).toBe('Recentfolders');
  expect(condition.querySelector('input[type="number"]')).not.toBeNull();
  expect(line.style.justifyContent).toBe('space-between');
});

it('shows the explainer while the question mark is hovered', () => {
  mount();
  const { help } = openedHelp();
  fireEvent.mouseEnter(help);
  expect(explainer()?.textContent).toBe(RECENT_EXPLAINED);
  fireEvent.mouseLeave(help);
  expect(explainer()).toBeNull();
});

it('shows the explainer while the question mark holds the focus', () => {
  mount();
  const { help } = openedHelp();
  act(() => help.focus());
  expect(explainer()?.textContent).toBe(RECENT_EXPLAINED);
  act(() => help.blur());
  expect(explainer()).toBeNull();
});

it('shows the explainer from a press on the question mark until a second press or a press elsewhere', () => {
  mount();
  const { help } = openedHelp();
  fireEvent.click(help);
  expect(explainer()?.textContent).toBe(RECENT_EXPLAINED);
  fireEvent.click(help);
  expect(explainer()).toBeNull();

  fireEvent.click(help);
  expect(explainer()).not.toBeNull();
  fireEvent.mouseDown(screen.getByRole('checkbox'));
  expect(explainer()).toBeNull();
  expect(screen.queryByRole('group', { name: 'More filters' })).not.toBeNull();
});

it('hides the explainer at the first Escape, leaving the panel open, and closes the panel at the next', () => {
  mount();
  const { help } = openedHelp();
  act(() => help.focus());
  fireEvent.click(help);
  expect(explainer()).not.toBeNull();
  expect(fireEvent.keyDown(help, { key: 'Escape' })).toBe(false);
  expect(explainer()).toBeNull();
  expect(screen.queryByRole('group', { name: 'More filters' })).not.toBeNull();

  fireEvent.keyDown(help, { key: 'Escape' });
  expect(screen.queryByRole('group', { name: 'More filters' })).toBeNull();
});

it('takes the focus by Ctrl+F and by /, but not from another input nor under a dialog', () => {
  mount();
  screen.getByRole('button', { name: 'elsewhere' }).focus();
  const find = fireEvent.keyDown(document.body, { key: 'f', ctrlKey: true });
  expect(find).toBe(false);
  expect(document.activeElement).toBe(input());

  screen.getByRole('button', { name: 'elsewhere' }).focus();
  expect(fireEvent.keyDown(document.activeElement as Element, { key: '/' })).toBe(false);
  expect(document.activeElement).toBe(input());

  // `/` typed into the field itself, or into another one, is a character.
  expect(fireEvent.keyDown(input(), { key: '/' })).toBe(true);
  const other = screen.getByRole('textbox', { name: 'another field' });
  other.focus();
  expect(fireEvent.keyDown(other, { key: '/' })).toBe(true);
  expect(fireEvent.keyDown(other, { key: 'f', ctrlKey: true })).toBe(true);
  expect(document.activeElement).toBe(other);

  // Nor while a modal dialog stands over the screen.
  screen.getByRole('button', { name: 'elsewhere' }).focus();
  const dialog = document.createElement('div');
  dialog.setAttribute('role', 'dialog');
  dialog.setAttribute('aria-modal', 'true');
  document.body.append(dialog);
  expect(fireEvent.keyDown(document.body, { key: '/' })).toBe(true);
  expect(fireEvent.keyDown(document.body, { key: 'f', ctrlKey: true })).toBe(true);
  expect(document.activeElement).not.toBe(input());
  dialog.remove();
});

it('hands the keys to the tree by ↓ or Enter', () => {
  const { onToTree } = mount();
  fireEvent.keyDown(input(), { key: 'ArrowDown' });
  fireEvent.keyDown(input(), { key: 'Enter' });
  expect(onToTree).toHaveBeenCalledTimes(2);
});

it('empties the name at the first Escape and gives the keys back at the next, leaving Recent on', () => {
  const { latest, onKeysBack } = mount({ name: 'star', recentOn: true, recent: 4 });
  const first = fireEvent.keyDown(input(), { key: 'Escape' });
  expect(first).toBe(false);
  expect(latest()).toEqual({ name: '', recentOn: true, recent: 4 });
  expect(onKeysBack).not.toHaveBeenCalled();
  fireEvent.keyDown(input(), { key: 'Escape' });
  expect(onKeysBack).toHaveBeenCalledTimes(1);
  expect(latest().recentOn).toBe(true);
});
