import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { addedFile, type Added } from '@coffret/api';

import { destinationsOf, type Destination } from './destination';
import { DropConfirm } from './DropConfirm';
import { summarize, type DropSummary, type HeldFiles } from './packChoice';
import { NO_NUMBERING } from './renumber';

afterEach(() => {
  cleanup();
  // The boxes are remembered across drops, which here would be across cases.
  window.localStorage.clear();
});

const BOOK: DropSummary = {
  folders: [{ name: 'BookX', files: 80, bytes: 58_000_000, exists: false }],
  loose: 0,
  looseBytes: 0,
  files: 80,
};

/** Where `BOOK` lands: under `comics`, which the Library has. */
const INTO: Destination[] = destinationsOf('comics', BOOK, (path) => path === 'comics');

/** The eighty files of `BOOK`: seventy-nine numbered pages and a cover. */
const FILES: Added[] = [
  ...Array.from({ length: 79 }, (_, at) =>
    addedFile(`BookX/king01-#${String(at + 1).padStart(3, '0')}.JPG`, { size: 10, lastModified: 0 } as File),
  ),
  addedFile('BookX/cover.png', { size: 10, lastModified: 0 } as File),
];

it('names the folder with its count and size, and offers a Pack or one by one', () => {
  const onChoose = vi.fn();
  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);

  expect(screen.getByRole('dialog').textContent).toContain('📁 BookX — 80 files, 58.0 MB');
  // A Pack unless somebody says otherwise.
  expect(screen.getByLabelText<HTMLInputElement>('As a Pack (recommended)').checked).toBe(true);
  fireEvent.click(add());
  fireEvent.click(screen.getByLabelText('The 80 files one by one'));
  fireEvent.click(add());
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(onChoose.mock.calls).toEqual([
    ['pack', NO_NUMBERING],
    ['one_by_one', NO_NUMBERING],
    ['cancel', NO_NUMBERING],
  ]);
});

/** The button that sends the drop. */
function add(): HTMLElement {
  return screen.getByRole('button', { name: 'Add' });
}

// Where the drop lands is said as the Library will hold it, with the level the
// drop makes marked as new.
it('says where the dropped folder lands, and that it is new', () => {
  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
  const said = screen.getByRole('dialog');
  expect(said.textContent).toContain('goes into comics/BookX/');
  expect(said.textContent).toContain('BookX is new — the Library has no such folder yet');
  const marked = [...said.querySelectorAll('[data-new]')].map((level) => level.textContent);
  expect(marked).toEqual(['BookX/']);
});

// A drop of files alone is confirmed too: how many, how much, that they go one
// by one, and where — and nothing goes until Add.
describe('a drop of files alone', () => {
  const LOOSE: Added[] = [
    addedFile('cover.png', { size: 1_500_000, lastModified: 0 } as File),
    addedFile('notes.txt', { size: 500_000, lastModified: 0 } as File),
  ];
  const summary = summarize(LOOSE, []);

  it('says how many, how much, where, and that they are added one by one', () => {
    const onChoose = vi.fn();
    render(
      <DropConfirm
        summary={summary}
        files={LOOSE}
        destinations={destinationsOf('comics/Foo', summary, () => true)}
        onChoose={onChoose}
      />,
    );
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain('2 files, 2.0 MB — added one by one');
    expect(said).toContain('goes into comics/Foo/');
    expect(said).not.toContain('is new');
    // No Pack to choose and nothing to number: they are no folder's pages.
    expect(screen.queryByRole('radio')).toBeNull();
    expect(screen.queryByLabelText('Number the pages')).toBeNull();
    expect(onChoose).not.toHaveBeenCalled();

    fireEvent.click(add());
    expect(onChoose).toHaveBeenCalledWith('one_by_one', NO_NUMBERING);
  });

  it('names the top of the Library where they are dropped there', () => {
    render(
      <DropConfirm
        summary={summary}
        files={LOOSE}
        destinations={destinationsOf('', summary, () => false)}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByRole('dialog').textContent).toContain('goes into the top of the Library');
  });

  it('sends nothing on Cancel, and numbers nothing whatever the last drop ticked', () => {
    window.localStorage.setItem('coffret.drop.number-pages', 'true');
    const onChoose = vi.fn();
    render(
      <DropConfirm
        summary={summary}
        files={LOOSE}
        destinations={destinationsOf('', summary, () => true)}
        onChoose={onChoose}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onChoose.mock.calls).toEqual([['cancel', NO_NUMBERING]]);
  });
});

// What the desktop app met in the drop and did not take is said in the
// confirmation, whether the drop goes straight in or is asked about as a Pack
// (spec: LA-26); a browser's drop has nothing to say there.
describe('what the desktop app did not take', () => {
  const LINK = [{ path: 'link-to-photos', reason: 'not_a_file_or_folder' }];
  const LINE =
    '1 thing in the drop was not taken: link-to-photos is a link or something else that is neither a file nor a folder';
  const LOOSE: Added[] = [addedFile('cover.png', { size: 10, lastModified: 0 } as File)];
  const loose = summarize(LOOSE, []);

  it('is said where files alone are added', () => {
    render(
      <DropConfirm
        summary={loose}
        files={LOOSE}
        destinations={destinationsOf('', loose, () => true)}
        notTaken={LINK}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByRole('dialog').textContent).toContain(LINE);
  });

  it('is said where the drop is asked what it goes in as', () => {
    render(
      <DropConfirm summary={BOOK} files={FILES} destinations={INTO} notTaken={LINK} onChoose={() => undefined} />,
    );
    expect(screen.getByRole('dialog').textContent).toContain(LINE);
    expect(screen.getByLabelText('As a Pack (recommended)')).toBeTruthy();
  });

  it('is not said where nothing was left out', () => {
    render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
    expect(screen.getByRole('dialog').textContent).not.toContain('not taken');
  });
});

// A folder the Library already has where the drop lands is said to exist, and
// the same answers stand.
it('says a dropped folder already exists here, and still offers the answers', () => {
  render(
    <DropConfirm
      summary={{ ...BOOK, folders: [{ ...BOOK.folders[0], exists: true }] }}
      files={FILES}
      destinations={INTO}
      onChoose={() => undefined}
    />,
  );
  expect(screen.getByRole('dialog').textContent).toContain('BookX already exists here');
  expect(screen.getByLabelText('As a Pack (recommended)')).toBeTruthy();
  expect(add()).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Cancel' })).toBeTruthy();
});

it('is cancelled by Escape, as any dialog is', () => {
  const onChoose = vi.fn();
  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onChoose).toHaveBeenCalledWith('cancel', NO_NUMBERING);
});

// Off unless somebody ticked them: a drop names its files the way it always
// did until a person asks for anything else.
it('offers both renames unticked, and shows nothing of them until one is ticked', () => {
  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
  const pages = screen.getByLabelText<HTMLInputElement>('Number the pages');
  const volumes = screen.getByLabelText<HTMLInputElement>('Number the volume folders');
  expect(pages.checked).toBe(false);
  expect(volumes.checked).toBe(false);
  expect(screen.getByRole('dialog').textContent).not.toContain('→');
});

it('previews what ticking a box renames, and carries it to the answer', () => {
  const onChoose = vi.fn();
  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);
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

  fireEvent.click(add());
  expect(onChoose).toHaveBeenCalledWith('pack', { pages: true, volumes: false });
});

// A person scanning books drops one after another and means the same each time.
it('remembers a ticked box for the next drop', () => {
  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
  fireEvent.click(screen.getByLabelText('Number the volume folders'));
  cleanup();

  render(<DropConfirm summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
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
    render(<DropConfirm summary={EXISTING} files={FILES} destinations={INTO} held={held} onChoose={() => undefined} />);
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
    render(<DropConfirm summary={EXISTING} files={FILES} destinations={INTO} held={held} onChoose={() => undefined} />);
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
    render(<DropConfirm summary={EXISTING} files={FILES} destinations={INTO} held={held} onChoose={() => undefined} />);
    expect(screen.getByRole('dialog').textContent).not.toContain('refused, because');

    // Numbered, two pages land on paths a Pack holds.
    fireEvent.click(screen.getByLabelText('Number the pages'));
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain('1 file in this drop replaces the file the Library already has');
    expect(said).toContain('2 files are refused, because a Pack holds the file at that path');
  });
});
