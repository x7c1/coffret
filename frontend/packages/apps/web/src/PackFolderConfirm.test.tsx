import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { PackFolderConfirm } from './PackFolderConfirm';
import { questionOf, type PackQuestion } from './packFolder';

afterEach(cleanup);

const BOOK: PackQuestion = {
  folder: 'books/BookX',
  line: '📁 BookX — 80 files, 58.0 MB will be packed into Packs',
  packs: true,
  leftOut: 'left as they are: 2 files already in a Pack',
  waits: null,
};

it('shows what will be packed, and offers Pack and Cancel', () => {
  const onChoose = vi.fn();
  render(<PackFolderConfirm question={BOOK} onChoose={onChoose} />);

  const dialog = screen.getByRole('dialog').textContent;
  expect(dialog).toContain('📁 BookX — 80 files, 58.0 MB will be packed into Packs');
  expect(dialog).toContain('left as they are: 2 files already in a Pack');
  fireEvent.click(screen.getByText('Pack'));
  fireEvent.click(screen.getByText('Cancel'));
  expect(onChoose.mock.calls).toEqual([[true], [false]]);
});

it('says a running freeze goes first', () => {
  render(
    <PackFolderConfirm
      question={{ ...BOOK, waits: 'a book is being packed already — this folder is packed after it' }}
      onChoose={() => undefined}
    />,
  );
  expect(screen.getByRole('dialog').textContent).toContain('this folder is packed after it');
});

// Nothing to pack: the reason, and nothing to press but Close.
it('offers no Pack where there is nothing to pack', () => {
  const onChoose = vi.fn();
  render(
    <PackFolderConfirm
      question={{
        ...BOOK,
        line: '📁 BookX — nothing to pack: 80 files already in a Pack',
        packs: false,
        leftOut: null,
      }}
      onChoose={onChoose}
    />,
  );
  expect(screen.queryByText('Pack')).toBeNull();
  fireEvent.click(screen.getByText('Close'));
  expect(onChoose).toHaveBeenCalledWith(false);
});

// The folder's own freeze is running or waiting: said so, with no later run
// promised and nothing to press but Close.
it('offers only Close for a folder being packed already', () => {
  const onChoose = vi.fn();
  const question = questionOf({
    folder: 'books/BookX',
    files: 80,
    bytes: 58_000_000,
    in_pack: 0,
    changed_in_pack: 0,
    not_here: 0,
    unavailable: 0,
    after_current: true,
    already_packing: true,
  });
  render(<PackFolderConfirm question={question} onChoose={onChoose} />);

  const dialog = screen.getByRole('dialog').textContent;
  expect(dialog).toContain('this folder is being packed already');
  expect(dialog).not.toContain('packed after it');
  expect(screen.queryByText('Pack')).toBeNull();
  fireEvent.click(screen.getByText('Close'));
  expect(onChoose).toHaveBeenCalledWith(false);
});

it('is cancelled by Escape, as any dialog is', () => {
  const onChoose = vi.fn();
  render(<PackFolderConfirm question={BOOK} onChoose={onChoose} />);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onChoose).toHaveBeenCalledWith(false);
});
