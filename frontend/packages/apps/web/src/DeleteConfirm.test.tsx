import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { DeleteConfirm } from './DeleteConfirm';
import { NOT_UNDOABLE, type DeleteQuestion } from './deleteEntries';

afterEach(cleanup);

const ALBUMS: DeleteQuestion = {
  target: { folder: 'albums', paths: [] },
  title: 'Delete 📁 albums?',
  removes: '12 files, 340.0 MB will be removed from the Library',
  rebuilds: '2 Packs holding other files will be rebuilt — reads 1.8 GB, writes 1.5 GB',
  refused: ['page-002.jpg stays in the Library — the Library has no key for the Pack holding them'],
  missing: null,
  waits: null,
  deletes: true,
};

// Everything the person is owed before saying yes: what leaves the Library,
// what a rebuild costs, what is refused and why, and that it cannot be undone.
it('shows what will be removed, rebuilt and refused, and that it cannot be undone', () => {
  render(<DeleteConfirm question={ALBUMS} onChoose={() => undefined} />);

  const dialog = screen.getByRole('dialog').textContent;
  expect(dialog).toContain('12 files, 340.0 MB will be removed from the Library');
  expect(dialog).toContain('2 Packs holding other files will be rebuilt — reads 1.8 GB, writes 1.5 GB');
  expect(dialog).toContain('the Library has no key for the Pack holding them');
  expect(dialog).toContain(NOT_UNDOABLE);
});

// Only Delete arms anything; Cancel, Escape and a click outside it are all no.
it('answers yes only to Delete', () => {
  const onChoose = vi.fn();
  render(<DeleteConfirm question={ALBUMS} onChoose={onChoose} />);

  fireEvent.click(screen.getByText('Delete'));
  fireEvent.click(screen.getByText('Cancel'));
  fireEvent.keyDown(window, { key: 'Escape' });
  fireEvent.click(screen.getByRole('presentation'));
  expect(onChoose.mock.calls).toEqual([[true], [false], [false], [false]]);
});

// An Enter pressed out of habit lands on Cancel, not on Delete.
it('puts the focus on Cancel', () => {
  render(<DeleteConfirm question={ALBUMS} onChoose={() => undefined} />);
  expect(document.activeElement?.textContent).toBe('Cancel');
});

it('offers no Delete where nothing would be removed', () => {
  render(
    <DeleteConfirm
      question={{ ...ALBUMS, removes: 'nothing will be removed from the Library', deletes: false }}
      onChoose={() => undefined}
    />,
  );
  expect(screen.queryByText('Delete')).toBeNull();
  expect(screen.getByText('Close')).toBeTruthy();
});
