import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Added } from '@coffret/api';

import { PackConfirm } from './PackConfirm';
import type { DropSummary, HeldFiles } from './packChoice';
import { NO_NUMBERING } from './renumber';

afterEach(() => {
  cleanup();
  // The boxes are remembered across drops, which here would be across cases.
  window.localStorage.clear();
});

const BOOK: DropSummary = {
  folders: [{ name: 'BookX', files: 80, bytes: 58_000_000, exists: false }],
  loose: 0,
  files: 80,
};

/** The eighty files of `BOOK`: seventy-nine numbered pages and a cover. */
const FILES: Added[] = [
  ...Array.from({ length: 79 }, (_, at) => ({
    path: `BookX/king01-#${String(at + 1).padStart(3, '0')}.JPG`,
    file: { size: 10 } as File,
  })),
  { path: 'BookX/cover.png', file: { size: 10 } as File },
];

it('names the folder with its count and size, and offers the three answers', () => {
  const onChoose = vi.fn();
  render(<PackConfirm summary={BOOK} files={FILES} onChoose={onChoose} />);

  expect(screen.getByRole('dialog').textContent).toContain('📁 BookX — 80 files, 58.0 MB');
  fireEvent.click(screen.getByText('Add as a Pack (recommended)'));
  fireEvent.click(screen.getByText('Add the 80 files one by one'));
  fireEvent.click(screen.getByText('Cancel'));
  expect(onChoose.mock.calls).toEqual([
    ['pack', NO_NUMBERING],
    ['one_by_one', NO_NUMBERING],
    ['cancel', NO_NUMBERING],
  ]);
});

// A folder the Library already has where the drop lands is said to exist, and
// the same answers stand.
it('says a dropped folder already exists here, and still offers the answers', () => {
  render(
    <PackConfirm
      summary={{ ...BOOK, folders: [{ ...BOOK.folders[0], exists: true }] }}
      files={FILES}
      onChoose={() => undefined}
    />,
  );
  expect(screen.getByRole('dialog').textContent).toContain('BookX already exists here');
  expect(screen.getByText('Add as a Pack (recommended)')).toBeTruthy();
  expect(screen.getByText('Cancel')).toBeTruthy();
});

it('is cancelled by Escape, as any dialog is', () => {
  const onChoose = vi.fn();
  render(<PackConfirm summary={BOOK} files={FILES} onChoose={onChoose} />);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onChoose).toHaveBeenCalledWith('cancel', NO_NUMBERING);
});

// Off unless somebody ticked them: a drop names its files the way it always
// did until a person asks for anything else.
it('offers both renames unticked, and shows nothing of them until one is ticked', () => {
  render(<PackConfirm summary={BOOK} files={FILES} onChoose={() => undefined} />);
  const pages = screen.getByLabelText<HTMLInputElement>('Number the pages');
  const volumes = screen.getByLabelText<HTMLInputElement>('Number the volume folders');
  expect(pages.checked).toBe(false);
  expect(volumes.checked).toBe(false);
  expect(screen.getByRole('dialog').textContent).not.toContain('→');
});

it('previews what ticking a box renames, and carries it to the answer', () => {
  const onChoose = vi.fn();
  render(<PackConfirm summary={BOOK} files={FILES} onChoose={onChoose} />);
  fireEvent.click(screen.getByLabelText('Number the pages'));

  const preview = screen.getByLabelText('what is renamed').textContent ?? '';
  expect(preview).toContain('king01-#001.JPG → 001.jpg');
  expect(preview).toContain('king01-#079.JPG → 079.jpg');
  expect(preview).toContain('73 more');
  expect(preview).not.toContain('king01-#040.JPG');
  expect(preview).toContain('cover.png (unchanged)');

  fireEvent.click(screen.getByText('Show all'));
  expect(screen.getByLabelText('what is renamed').textContent).toContain(
    'king01-#040.JPG → 040.jpg',
  );

  fireEvent.click(screen.getByText('Add as a Pack (recommended)'));
  expect(onChoose).toHaveBeenCalledWith('pack', { pages: true, volumes: false });
});

// A person scanning books drops one after another and means the same each time.
it('remembers a ticked box for the next drop', () => {
  render(<PackConfirm summary={BOOK} files={FILES} onChoose={() => undefined} />);
  fireEvent.click(screen.getByLabelText('Number the volume folders'));
  cleanup();

  render(<PackConfirm summary={BOOK} files={FILES} onChoose={() => undefined} />);
  expect(screen.getByLabelText<HTMLInputElement>('Number the volume folders').checked).toBe(true);
  expect(screen.getByLabelText<HTMLInputElement>('Number the pages').checked).toBe(false);
});

// PK-10: where a dropped path already holds a file of the Library, the question
// says so before anything is sent — how many are replaced, naming the first
// few, and how many a Pack refuses. Where nothing collides it says no more than
// it did.
describe('what a drop onto a folder of the same name replaces', () => {
  const EXISTING: DropSummary = { ...BOOK, folders: [{ ...BOOK.folders[0], exists: true }] };

  it('says nothing more where no dropped path is held', () => {
    const held: HeldFiles = new Map([['BookX/elsewhere.jpg', 'one-file']]);
    render(<PackConfirm summary={EXISTING} files={FILES} held={held} onChoose={() => undefined} />);
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain('BookX already exists here');
    expect(said).not.toContain('in this drop replace');
    expect(said).not.toContain('refused, because');
  });

  it('names the files that replace one-file Entries', () => {
    const held: HeldFiles = new Map([
      ['BookX/cover.png', 'one-file'],
      ['BookX/king01-#001.JPG', 'one-file'],
    ]);
    render(<PackConfirm summary={EXISTING} files={FILES} held={held} onChoose={() => undefined} />);
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain(
      '2 files in this drop replace the files the Library already has at the same path: ' +
        'BookX/king01-#001.JPG, BookX/cover.png',
    );
    expect(said).not.toContain('refused, because');
  });

  it('says how many a Pack refuses beside the ones replaced, for the paths the drop is sent under', () => {
    const held: HeldFiles = new Map([
      ['BookX/cover.png', 'one-file'],
      ['BookX/001.jpg', 'pack'],
      ['BookX/002.jpg', 'pack'],
    ]);
    render(<PackConfirm summary={EXISTING} files={FILES} held={held} onChoose={() => undefined} />);
    expect(screen.getByRole('dialog').textContent).not.toContain('refused, because');

    // Numbered, two pages land on paths a Pack holds.
    fireEvent.click(screen.getByLabelText('Number the pages'));
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain('1 file in this drop replaces the file the Library already has');
    expect(said).toContain('2 files are refused, because a Pack holds the file at that path');
  });
});
