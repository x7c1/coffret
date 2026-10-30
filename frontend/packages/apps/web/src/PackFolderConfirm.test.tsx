import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { PackFolderConfirm } from './PackFolderConfirm';
import { PACK_EXPLAINED, questionOf, type PackQuestion } from './packFolder';

afterEach(cleanup);

const BOOK: PackQuestion = {
  folder: 'books/BookX',
  title: 'Pack “BookX”?',
  line: '80 files, 58.0 MB will be packed',
  packs: true,
  leftOut: ['2 files already in a Pack', '3 files not on this device'],
  waits: null,
};

it('shows what will be packed and what is left as it is, and offers Pack and Cancel', () => {
  const onChoose = vi.fn();
  render(<PackFolderConfirm question={BOOK} onChoose={onChoose} />);

  const dialog = screen.getByRole('dialog', { name: 'Pack “BookX”?' }).textContent;
  expect(dialog).toContain('80 files, 58.0 MB will be packed');
  expect(dialog).toContain('Left as they are');
  expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
    '2 files already in a Pack',
    '3 files not on this device',
  ]);
  expect(dialog).toContain(PACK_EXPLAINED);
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
        line: 'Nothing to pack: 80 files already in a Pack.',
        packs: false,
        leftOut: [],
      }}
      onChoose={onChoose}
    />,
  );
  expect(screen.queryByText('Pack')).toBeNull();
  expect(screen.queryByText('Left as they are')).toBeNull();
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
    receiving: 0,
    system_files: 0,
    system_folders: 0,
    unavailable: 0,
    after_current: true,
    already_packing: true,
  });
  render(<PackFolderConfirm question={question} onChoose={onChoose} />);

  const dialog = screen.getByRole('dialog').textContent;
  expect(dialog).toContain('This folder is being packed already.');
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

// PK-17: the files of a drop not finished bringing them in are left for that
// drop's own freeze, and the dialog names them among what is left as it is.
it('lists the files on their way in under what is left as it is', () => {
  const question = questionOf({
    folder: 'books/BookX',
    files: 80,
    bytes: 58_000_000,
    in_pack: 0,
    changed_in_pack: 0,
    not_here: 0,
    receiving: 4,
    system_files: 0,
    system_folders: 0,
    unavailable: 0,
    after_current: false,
    already_packing: false,
  });
  render(<PackFolderConfirm question={question} onChoose={() => undefined} />);

  expect(screen.getByRole('dialog').textContent).toContain('Left as they are');
  expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
    '4 files not in the Library yet',
  ]);
});
