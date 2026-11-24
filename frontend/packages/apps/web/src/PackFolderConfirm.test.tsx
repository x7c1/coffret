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
  inTheWay: [],
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
    file_and_folder: 0,
    in_the_way: [],
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
    file_and_folder: 0,
    in_the_way: [],
    after_current: false,
    already_packing: false,
  });
  render(<PackFolderConfirm question={question} onChoose={() => undefined} />);

  expect(screen.getByRole('dialog').textContent).toContain('Left as they are');
  expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
    '4 files not in the Library yet',
  ]);
});

// EP-6: a freeze any file of which would make a path both a file and a folder
// is refused whole, so the dialog says the folder cannot be packed and why —
// each file in the way with what to rename, the mapped folder never offered —
// and gives nothing to press but Close.
it('says a freeze that would be refused cannot run and why, and offers no Pack', () => {
  const onChoose = vi.fn();
  const message =
    'adding this file would make "notes" both a file and a folder, and the folder is the one ' +
    'this device maps at "notes", so renaming that folder on disk changes nothing; rename the ' +
    'file "notes", then pack again';
  const question = questionOf({
    folder: 'notes',
    files: 3,
    bytes: 3_000,
    in_pack: 0,
    changed_in_pack: 0,
    not_here: 0,
    receiving: 0,
    system_files: 0,
    system_folders: 0,
    unavailable: 0,
    file_and_folder: 1,
    in_the_way: [{ path: 'notes/x.pdf', message }],
    after_current: false,
    already_packing: false,
  });
  render(<PackFolderConfirm question={question} onChoose={onChoose} />);

  const dialog = screen.getByRole('dialog', { name: 'Pack “notes”?' }).textContent;
  expect(dialog).toContain(
    'Can’t pack this folder: 1 file would make a path both a file and a folder, so nothing ' +
      'would be packed.',
  );
  expect(dialog).toContain('In the way');
  expect(screen.getAllByRole('listitem').map((item) => item.textContent)).toEqual([
    `notes/x.pdf — ${message}`,
  ]);
  expect(dialog).not.toContain('will be packed');
  expect(dialog).not.toContain('rename the file or the folder');
  expect(screen.queryByText('Pack')).toBeNull();
  expect(screen.queryByText('Cancel')).toBeNull();
  fireEvent.click(screen.getByText('Close'));
  expect(onChoose).toHaveBeenCalledWith(false);
});
