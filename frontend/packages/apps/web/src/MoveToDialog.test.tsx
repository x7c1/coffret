import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import type { Described } from './details';
import { MoveToDialog } from './MoveToDialog';
import { fileKey } from './tree';

afterEach(cleanup);

const COVER = { kind: 'file', key: fileKey('series/cover.jpg'), path: 'series/cover.jpg', name: 'cover.jpg' } as unknown as Described;

it('offers the Library and the folders, opened to where the rows stand, and goes on with the one chosen', () => {
  const onChoose = vi.fn();
  render(
    <MoveToDialog
      title="Move “cover.jpg” to…"
      folders={['albums', 'series', 'series/vol-1']}
      moved={[COVER]}
      onChoose={onChoose}
      onClose={vi.fn()}
    />,
  );
  const tree = within(screen.getByRole('tree', { name: 'folders' }));
  expect(tree.getAllByRole('treeitem').map((item) => item.textContent)).toEqual(['Library', 'albums', 'series', 'vol-1']);
  // The folder the cover stands in takes nothing.
  expect(tree.getByTitle('series').getAttribute('aria-disabled')).toBe('true');
  const move = screen.getByRole('button', { name: 'Move' });
  expect(move).toHaveProperty('disabled', true);
  fireEvent.click(tree.getByTitle('series'));
  expect(move).toHaveProperty('disabled', true);
  fireEvent.click(tree.getByTitle('series/vol-1'));
  fireEvent.click(move);
  expect(onChoose).toHaveBeenCalledWith('series/vol-1');
  fireEvent.click(tree.getByRole('button', { name: 'collapse series' }));
  expect(tree.getAllByRole('treeitem').map((item) => item.textContent)).toEqual(['Library', 'albums', 'series']);
});

it('walks the folders by keyboard: one stop of Tab, ↑/↓ between rows, →/← to open and close, Enter or Space to choose', () => {
  const onChoose = vi.fn();
  render(
    <MoveToDialog
      title="Move “cover.jpg” to…"
      folders={['albums', 'series', 'series/vol-1', 'series/vol-1/extras']}
      moved={[COVER]}
      onChoose={onChoose}
      onClose={vi.fn()}
    />,
  );
  const tree = within(screen.getByRole('tree', { name: 'folders' }));
  const tabbable = () => tree.getAllByRole('treeitem').filter((item) => item.tabIndex === 0);
  const names = () => tree.getAllByRole('treeitem').map((item) => item.textContent);
  const press = (key: string) => fireEvent.keyDown(document.activeElement ?? document.body, { key });
  // Nothing chosen yet: the Library row is the one stop, and no disclosure is one.
  expect(tabbable().map((item) => item.textContent)).toEqual(['Library']);
  expect(tree.getAllByRole('button').every((button) => button.tabIndex === -1)).toBe(true);
  tree.getByTitle('Library').focus();
  press('ArrowDown');
  expect(document.activeElement).toBe(tree.getByTitle('albums'));
  press('ArrowDown');
  expect(document.activeElement).toBe(tree.getByTitle('series'));
  expect(tabbable()).toEqual([tree.getByTitle('series')]);
  // An open folder: → goes to its first folder, which → then opens.
  press('ArrowRight');
  expect(document.activeElement).toBe(tree.getByTitle('series/vol-1'));
  expect(names()).toEqual(['Library', 'albums', 'series', 'vol-1']);
  press('ArrowRight');
  expect(names()).toEqual(['Library', 'albums', 'series', 'vol-1', 'extras']);
  expect(document.activeElement).toBe(tree.getByTitle('series/vol-1'));
  // ← closes it, and from a closed one goes to the folder it stands in.
  press('ArrowLeft');
  expect(names()).toEqual(['Library', 'albums', 'series', 'vol-1']);
  press('ArrowLeft');
  expect(document.activeElement).toBe(tree.getByTitle('series'));
  // The folder the cover stands in cannot be chosen; Enter on it does nothing.
  press('Enter');
  expect(screen.getByRole('button', { name: 'Move' })).toHaveProperty('disabled', true);
  press('ArrowUp');
  press(' ');
  expect(tree.getByTitle('albums').getAttribute('aria-selected')).toBe('true');
  press('ArrowDown');
  press('ArrowRight');
  press('Enter');
  expect(tree.getByTitle('series/vol-1').getAttribute('aria-selected')).toBe('true');
  fireEvent.click(screen.getByRole('button', { name: 'Move' }));
  expect(onChoose).toHaveBeenCalledWith('series/vol-1');
});

it('keeps the Library row unchosen by keyboard where the rows already stand at the root, and starts Tab on the chosen folder', () => {
  const atRoot = { kind: 'file', key: fileKey('cover.jpg'), path: 'cover.jpg', name: 'cover.jpg' } as unknown as Described;
  render(
    <MoveToDialog title="Move “cover.jpg” to…" folders={['albums']} moved={[atRoot]} onChoose={vi.fn()} onClose={vi.fn()} />,
  );
  const tree = within(screen.getByRole('tree', { name: 'folders' }));
  const library = tree.getByTitle('Library');
  expect(library.getAttribute('aria-disabled')).toBe('true');
  library.focus();
  fireEvent.keyDown(library, { key: 'Enter' });
  fireEvent.keyDown(library, { key: ' ' });
  expect(library.getAttribute('aria-selected')).toBe('false');
  expect(library.tabIndex).toBe(0);
  fireEvent.click(tree.getByTitle('albums'));
  expect(tree.getByTitle('albums').tabIndex).toBe(0);
  expect(library.tabIndex).toBe(-1);
});
