import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { PackConfirm } from './PackConfirm';
import type { DropSummary } from './packChoice';

afterEach(cleanup);

const BOOK: DropSummary = {
  folders: [{ name: 'BookX', files: 80, bytes: 58_000_000, exists: false }],
  loose: 0,
  files: 80,
};

it('names the folder with its count and size, and offers the three answers', () => {
  const onChoose = vi.fn();
  render(<PackConfirm summary={BOOK} onChoose={onChoose} />);

  expect(screen.getByRole('dialog').textContent).toContain('📁 BookX — 80 files, 58.0 MB');
  fireEvent.click(screen.getByText('Add as a Pack (recommended)'));
  fireEvent.click(screen.getByText('Add the 80 files one by one'));
  fireEvent.click(screen.getByText('Cancel'));
  expect(onChoose.mock.calls).toEqual([['pack'], ['one_by_one'], ['cancel']]);
});

// A folder the Library already has where the drop lands is said to exist, and
// the same answers stand.
it('says a dropped folder already exists here, and still offers the answers', () => {
  render(
    <PackConfirm
      summary={{ ...BOOK, folders: [{ ...BOOK.folders[0], exists: true }] }}
      onChoose={() => undefined}
    />,
  );
  expect(screen.getByRole('dialog').textContent).toContain('BookX already exists here');
  expect(screen.getByText('Add as a Pack (recommended)')).toBeTruthy();
  expect(screen.getByText('Cancel')).toBeTruthy();
});

it('is cancelled by Escape, as any dialog is', () => {
  const onChoose = vi.fn();
  render(<PackConfirm summary={BOOK} onChoose={onChoose} />);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onChoose).toHaveBeenCalledWith('cancel');
});
