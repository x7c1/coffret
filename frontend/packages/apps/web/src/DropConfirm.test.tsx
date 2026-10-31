import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { addedFile, type Added } from '@coffret/api';

import { destinationsOf, type Destination } from './destination';
import { DropConfirm } from './DropConfirm';
import { NOTHING_HELD, summarize, type DropSummary, type HeldFiles } from './packChoice';
import { LINES_DRAWN, NO_EDITS, NO_NUMBERING } from './renumber';
import { COLOR } from './theme';

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

it('says where it adds, names the folder with its count and size, and offers One Pack or separate files', () => {
  const onChoose = vi.fn();
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);

  const said = screen.getByRole('dialog', { name: 'Add to comics' });
  expect(said.textContent).toContain('80 files, 58.0 MB');
  expect(said.textContent).toContain('📁 BookX — 80 files, 58.0 MB');
  // One Pack unless somebody says otherwise, each choice saying what it is for.
  expect(packChoice().checked).toBe(true);
  expect(said.textContent).toContain('One Pack — kept and fetched together, best for a book or an album');
  expect(said.textContent).toContain(
    'Separate files — each file kept on its own, best for files you will replace',
  );
  fireEvent.click(add());
  fireEvent.click(screen.getByRole('radio', { name: /^Separate files/ }));
  fireEvent.click(add());
  fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
  expect(onChoose.mock.calls).toEqual([
    ['pack', NO_NUMBERING, NO_EDITS],
    ['one_by_one', NO_NUMBERING, NO_EDITS],
    ['cancel', NO_NUMBERING, NO_EDITS],
  ]);
});

/** The button that sends the drop. */
function add(): HTMLElement {
  return screen.getByRole('button', { name: 'Add' });
}

/** The choice of storing a dropped folder as One Pack. */
function packChoice(): HTMLInputElement {
  return screen.getByRole<HTMLInputElement>('radio', { name: /^One Pack/ });
}

/** The marks the incoming items carry, top to bottom. */
function marks(): string[] {
  const list = screen.getByRole('list', { name: 'incoming' });
  return [...list.querySelectorAll('li')].map((item) => item.textContent ?? '');
}

// A dropped folder the Library has no folder of where it lands is marked as
// a new folder.
it('marks a dropped folder the Library does not have there as a new folder', () => {
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
  expect(marks()).toEqual(['New folder📁 BookX — 80 files, 58.0 MB']);
});

// Where the folder added to is not one the Library holds yet — prepared with
// "New folder…" — the confirmation says the drop makes it.
it('says the drop makes a folder added to that the Library does not hold yet', () => {
  const prepared = destinationsOf('comics/New', BOOK, (path) => path === 'comics');
  render(<DropConfirm landing="comics/New" summary={BOOK} files={FILES} destinations={prepared} onChoose={() => undefined} />);
  const said = screen.getByRole('dialog', { name: 'Add to comics/New' });
  expect(said.textContent).toContain('New is new — the Library has no such folder yet');
});

// A dropped folder already standing on this device's disk is not new: the
// confirmation says what it is and that what is in it goes in too.
it('says a dropped folder standing on disk is on disk and not in the Library yet', () => {
  const onDisk = destinationsOf(
    'comics',
    BOOK,
    (path) => path === 'comics',
    (path) => path === 'comics/BookX',
  );
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={onDisk} onChoose={() => undefined} />);
  const said = screen.getByRole('dialog');
  expect(said.textContent).toContain(
    'BookX is on this device’s disk and not in the Library yet — it is added together ' +
      'with what is already in it',
  );
  expect(said.textContent).not.toContain('is new');
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
      <DropConfirm landing="comics/Foo"
        summary={summary}
        files={LOOSE}
        destinations={destinationsOf('comics/Foo', summary, () => true)}
        onChoose={onChoose}
      />,
    );
    const said = screen.getByRole('dialog', { name: 'Add to comics/Foo' }).textContent ?? '';
    expect(said).toContain('2 files, 2.0 MB');
    expect(marks()).toEqual(['Newcover.png — 1.5 MB', 'Newnotes.txt — 500.0 kB']);
    expect(said).not.toContain('is new');
    // No Pack to choose and nothing to number: they are no folder's pages.
    expect(screen.queryByRole('radio')).toBeNull();
    expect(screen.queryByLabelText('Number the pages')).toBeNull();
    expect(onChoose).not.toHaveBeenCalled();

    fireEvent.click(add());
    expect(onChoose).toHaveBeenCalledWith('one_by_one', NO_NUMBERING, NO_EDITS);
  });

  it('names the Library where they are dropped at its root', () => {
    render(
      <DropConfirm landing=""
        summary={summary}
        files={LOOSE}
        destinations={destinationsOf('', summary, () => false)}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByRole('dialog', { name: 'Add to the Library' })).toBeTruthy();
  });

  it('sends nothing on Cancel, and numbers nothing whatever the last drop ticked', () => {
    window.localStorage.setItem('coffret.drop.number-pages', 'true');
    const onChoose = vi.fn();
    render(
      <DropConfirm landing=""
        summary={summary}
        files={LOOSE}
        destinations={destinationsOf('', summary, () => true)}
        onChoose={onChoose}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onChoose.mock.calls).toEqual([['cancel', NO_NUMBERING, NO_EDITS]]);
  });
});

// What the desktop app met in the drop and did not take is said in the
// confirmation, whether the drop goes straight in or is asked about as a Pack
// (spec: LA-26); a browser's drop has nothing to say there.
describe('what the desktop app did not take', () => {
  const LINK = [{ path: 'link-to-photos', reason: 'not_a_file_or_folder' }];
  const LINE =
    '1 thing was not taken: link-to-photos is a link or something else that is neither a file nor a folder';
  const LOOSE: Added[] = [addedFile('cover.png', { size: 10, lastModified: 0 } as File)];
  const loose = summarize(LOOSE, []);

  it('is said where files alone are added', () => {
    render(
      <DropConfirm landing=""
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
      <DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} notTaken={LINK} onChoose={() => undefined} />,
    );
    expect(screen.getByRole('dialog').textContent).toContain(LINE);
    expect(packChoice()).toBeTruthy();
  });

  it('is not said where nothing was left out', () => {
    render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
    expect(screen.getByRole('dialog').textContent).not.toContain('not taken');
  });
});

// What either walk left out because an operating system keeps it is said in
// one dim line of its own, naming the distinct names, and never in the warning
// line about what was not taken (spec: EP-17).
describe('what the system keeps', () => {
  const KEPT = [
    { path: 'BookX/.DS_Store', folder: false },
    { path: 'BookX/sub/.DS_Store', folder: false },
    { path: 'BookX/Thumbs.db', folder: false },
  ];
  const LINE = 'left out 3 files the system keeps (.DS_Store, Thumbs.db)';
  const LINK = [{ path: 'link-to-photos', reason: 'not_a_file_or_folder' }];

  it('is said in the dim colour, apart from what was not taken', () => {
    render(
      <DropConfirm landing="comics"
        summary={BOOK}
        files={FILES}
        destinations={INTO}
        notTaken={LINK}
        systemFiles={KEPT}
        onChoose={() => undefined}
      />,
    );
    const kept = screen.getByText(LINE);
    expect(kept.style.color).toBe(hex(COLOR.dim));
    const warned = screen.getByText(/^1 thing was not taken: link-to-photos/);
    expect(warned.style.color).toBe(hex(COLOR.warn));
    expect(warned.textContent).not.toContain('.DS_Store');
  });

  it('counts the folders it left out apart from the files', () => {
    render(
      <DropConfirm landing="comics"
        summary={BOOK}
        files={FILES}
        destinations={INTO}
        systemFiles={[...KEPT, { path: 'BookX/__MACOSX', folder: true }]}
        onChoose={() => undefined}
      />,
    );
    expect(screen.getByRole('dialog').textContent).toContain(
      'left out 3 files and 1 folder the system keeps (.DS_Store, Thumbs.db, __MACOSX)',
    );
  });

  it('is not said where nothing was left out', () => {
    render(
      <DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} notTaken={LINK} onChoose={() => undefined} />,
    );
    const said = screen.getByRole('dialog').textContent;
    expect(said).not.toContain('the system keeps');
    expect(said).toContain('1 thing was not taken');
  });
});

/** A colour as the DOM reads an inline style back. */
function hex(colour: string): string {
  const probe = document.createElement('span');
  probe.style.color = colour;
  return probe.style.color;
}

// A folder the Library already has where the drop lands is said to exist, and
// the same answers stand.
it('says a dropped folder already exists here, and still offers the answers', () => {
  render(
    <DropConfirm landing="comics"
      summary={{ ...BOOK, folders: [{ ...BOOK.folders[0], exists: true }] }}
      files={FILES}
      destinations={INTO}
      onChoose={() => undefined}
    />,
  );
  expect(screen.getByRole('dialog').textContent).toContain('BookX already exists here');
  // No mark: the line says what it is.
  expect(marks()[0].startsWith('📁 BookX')).toBe(true);
  expect(packChoice()).toBeTruthy();
  expect(add()).toBeTruthy();
  expect(screen.getByRole('button', { name: 'Cancel' })).toBeTruthy();
});

it('is cancelled by Escape, as any dialog is', () => {
  const onChoose = vi.fn();
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onChoose).toHaveBeenCalledWith('cancel', NO_NUMBERING, NO_EDITS);
});

// Off unless somebody ticked them: a drop names its files the way it always
// did until a person asks for anything else.
it('offers both renames unticked, and shows nothing of them until one is ticked', () => {
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
  const pages = screen.getByLabelText<HTMLInputElement>('Number the pages');
  const volumes = screen.getByLabelText<HTMLInputElement>('Number the volume folders');
  expect(pages.checked).toBe(false);
  expect(volumes.checked).toBe(false);
  expect(screen.getByRole('dialog').textContent).not.toContain('→');
});

it('previews what ticking a box renames, and carries it to the answer', () => {
  const onChoose = vi.fn();
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);
  fireEvent.click(screen.getByLabelText('Number the pages'));

  const preview = screen.getByLabelText('what is renamed').textContent ?? '';
  expect(preview).toContain('80 renamed · 1 to check');
  // The cover put before the first page is irregular and always shown; the
  // first and last regular lines stand either side of the fold.
  expect(preview).toContain('cover.png → 000_cover.png');
  expect(preview).toContain('king01-#001.JPG → 001.jpg');
  expect(preview).toContain('king01-#079.JPG → 079.jpg');
  expect(preview).toContain('… 77 more …');
  expect(preview).not.toContain('king01-#040.JPG');

  fireEvent.click(screen.getByText('Show all'));
  expect(screen.getByLabelText('what is renamed').textContent).toContain(
    'king01-#040.JPG → 040.jpg',
  );

  fireEvent.click(add());
  expect(onChoose).toHaveBeenCalledWith('pack', { pages: true, volumes: false }, NO_EDITS);
});

// The names numbered to keep their place — a cover, a spread — are renames
// like any other, and a folder the rule cannot keep in order says why.
it('previews a prefixed name and a spread as renames, and a folder left whole', () => {
  const file = (path: string) => addedFile(path, { size: 10, lastModified: 0 } as File);
  const files = [
    file('BookX/cover.jpg'),
    file('BookX/p1.jpg'),
    file('BookX/p2+p3.jpg'),
    file('BookX/p4.jpg'),
    file('BookY/cover.jpg'),
    file('BookY/p0.jpg'),
    file('BookY/p1.jpg'),
  ];
  render(<DropConfirm landing="comics" summary={BOOK} files={files} destinations={INTO} onChoose={() => undefined} />);
  fireEvent.click(screen.getByLabelText('Number the pages'));

  const preview = screen.getByLabelText('what is renamed').textContent ?? '';
  expect(preview).toContain('cover.jpg → 000_cover.jpg');
  expect(preview).toContain('p2+p3.jpg → 002+003.jpg');
  // The line names the two names read out of order, so the person knows
  // which to rename.
  expect(preview).toContain(
    'left as they are: numbering them would change the order they are read in ' +
      '(cover.jpg is read before p0.jpg)',
  );
  expect(preview).toContain('p0.jpg (unchanged)');
});

// A page already named as its number stands among its neighbours, and the
// files that are no pages come after the pages.
it('lists renamed and unchanged names in the order they are read in', () => {
  const file = (path: string) => addedFile(path, { size: 10, lastModified: 0 } as File);
  const files = [
    file('BookX/ComicInfo.xml'),
    file('BookX/0.jpg'),
    file('BookX/001.jpg'),
    file('BookX/2.jpg'),
  ];
  render(<DropConfirm landing="comics" summary={BOOK} files={files} destinations={INTO} onChoose={() => undefined} />);
  fireEvent.click(screen.getByLabelText('Number the pages'));

  const lines = [...screen.getByLabelText('what is renamed').querySelectorAll('li')].map(
    (line) => line.textContent,
  );
  expect(lines).toEqual([
    '0.jpg → 000.jpg',
    '001.jpg (unchanged)',
    '2.jpg → 002.jpg',
    'ComicInfo.xml (unchanged)',
  ]);
});

// A person scanning books drops one after another and means the same each time.
it('remembers a ticked box for the next drop', () => {
  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
  fireEvent.click(screen.getByLabelText('Number the volume folders'));
  cleanup();

  render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={() => undefined} />);
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
    render(<DropConfirm landing="comics" summary={EXISTING} files={FILES} destinations={INTO} held={held} onChoose={() => undefined} />);
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain('BookX already exists here');
    expect(said).not.toContain('being added replace');
    expect(said).not.toContain('refused, because');
  });

  it('names the files that replace one-file Entries', () => {
    const held: HeldFiles = new Map([
      ['BookX/cover.png', 'one-file'],
      ['BookX/king01-#001.JPG', 'one-file'],
    ]);
    render(<DropConfirm landing="comics" summary={EXISTING} files={FILES} destinations={INTO} held={held} onChoose={() => undefined} />);
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain(
      '2 files being added replace the files the Library already has at the same path: ' +
        'BookX/king01-#001.JPG, BookX/cover.png',
    );
    expect(said).not.toContain('refused, because');
  });

  it('says how many a Pack refuses beside the ones replaced, for the paths the drop is sent under', () => {
    const held: HeldFiles = new Map([
      ['BookX/000_cover.png', 'one-file'],
      ['BookX/001.jpg', 'pack'],
      ['BookX/002.jpg', 'pack'],
    ]);
    render(<DropConfirm landing="comics" summary={EXISTING} files={FILES} destinations={INTO} held={held} onChoose={() => undefined} />);
    expect(screen.getByRole('dialog').textContent).not.toContain('refused, because');

    // Numbered, the cover lands on a one-file Entry and two pages on paths a
    // Pack holds.
    fireEvent.click(screen.getByLabelText('Number the pages'));
    const said = screen.getByRole('dialog').textContent ?? '';
    expect(said).toContain('1 file being added replaces the file the Library already has');
    expect(said).toContain('2 files are refused, because a Pack holds the file at that path');
  });
});

// The rule cannot read every book the way its owner meant, so a new name in
// the preview can be corrected before anything is sent.
describe('correcting a new name in the preview', () => {
  /** The confirmation of `BOOK` with its pages numbered, and what it answers with. */
  function numbered() {
    const onChoose = vi.fn();
    render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);
    fireEvent.click(screen.getByLabelText('Number the pages'));
    return onChoose;
  }

  /** The preview's text. */
  function preview(): string {
    return screen.getByLabelText('what is renamed').textContent ?? '';
  }

  /** Opens the new name `name` for correcting, types `typed` and presses `key`. */
  function correct(name: string, typed: string, key = 'Enter'): HTMLInputElement {
    fireEvent.click(screen.getByRole('button', { name }));
    const field = screen.getByRole<HTMLInputElement>('textbox');
    fireEvent.change(field, { target: { value: typed } });
    fireEvent.keyDown(field, { key });
    return field;
  }

  it('opens a new name for editing, takes it on Enter, marks it, and sends it on Add', () => {
    const onChoose = numbered();
    fireEvent.click(screen.getByRole('button', { name: '001.jpg' }));
    const field = screen.getByLabelText<HTMLInputElement>('new name of king01-#001.JPG');
    expect(field.value).toBe('001.jpg');

    fireEvent.change(field, { target: { value: '001 first.jpg' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(preview()).toContain('king01-#001.JPG → 001 first.jpg (edited)');
    expect(preview()).not.toContain('read out of order');
    // Enter in the field took the name and did not press Add.
    expect(onChoose).not.toHaveBeenCalled();

    // Leaving the field takes what was typed, as Enter does.
    fireEvent.click(screen.getByRole('button', { name: '002.jpg' }));
    const second = screen.getByRole('textbox');
    fireEvent.change(second, { target: { value: '002 second.jpg' } });
    fireEvent.blur(second);
    expect(preview()).toContain('king01-#002.JPG → 002 second.jpg (edited)');

    fireEvent.click(add());
    expect(onChoose).toHaveBeenCalledWith(
      'pack',
      { pages: true, volumes: false },
      new Map([
        ['BookX/king01-#001.JPG', '001 first.jpg'],
        ['BookX/king01-#002.JPG', '002 second.jpg'],
      ]),
    );
  });

  it('puts the field away on Escape without the edit, and without cancelling the dialog', () => {
    const onChoose = numbered();
    correct('001.jpg', 'typo.jpg', 'Escape');
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(preview()).toContain('king01-#001.JPG → 001.jpg');
    expect(preview()).not.toContain('(edited)');
    expect(onChoose).not.toHaveBeenCalled();
    expect(screen.getByRole('dialog')).toBeTruthy();
  });

  it('puts back the name the rule gave on Undo', () => {
    const onChoose = numbered();
    correct('001.jpg', 'one.jpg');
    expect(preview()).toContain('king01-#001.JPG → one.jpg (edited)');
    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(preview()).toContain('king01-#001.JPG → 001.jpg');
    expect(preview()).not.toContain('(edited)');
    fireEvent.click(add());
    expect(onChoose).toHaveBeenCalledWith('pack', { pages: true, volumes: false }, NO_EDITS);
  });

  // The control that had the focus goes away with Enter, Escape or Undo;
  // the focus lands on the name shown in its place rather than on nothing.
  it('gives the focus back to the name when the field or Undo goes away', () => {
    numbered();
    correct('001.jpg', 'one.jpg');
    expect(document.activeElement).toBe(screen.getByRole('button', { name: 'one.jpg' }));

    fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
    expect(document.activeElement).toBe(screen.getByRole('button', { name: '001.jpg' }));

    correct('079.jpg', 'typo.jpg', 'Escape');
    expect(document.activeElement).toBe(screen.getByRole('button', { name: '079.jpg' }));
  });

  // The field stays open and says why; Add sends the names last taken.
  it('keeps a refused name open with its reason', () => {
    const onChoose = numbered();
    const field = correct('001.jpg', '002.jpg');
    expect(screen.getByRole('textbox')).toBe(field);
    expect(preview()).toContain('there is already something with this name here');

    fireEvent.change(field, { target: { value: '' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(preview()).toContain('a name cannot be empty');

    fireEvent.change(field, { target: { value: 'x/001.jpg' } });
    fireEvent.blur(field);
    expect(screen.getByRole('textbox')).toBe(field);
    expect(preview()).toContain('a name cannot contain /');

    fireEvent.click(add());
    expect(onChoose).toHaveBeenCalledWith('pack', { pages: true, volumes: false }, NO_EDITS);
  });

  it('takes a name that breaks the reading order, and warns of it', () => {
    numbered();
    correct('001.jpg', '100.jpg');
    expect(preview()).toContain('king01-#001.JPG → 100.jpg (edited)');
    expect(preview()).toContain('read out of order: this name sorts after 002.jpg');
  });

  it.each(['Number the pages', 'Number the volume folders'])(
    'discards every edit when "%s" is toggled',
    (box) => {
      const onChoose = vi.fn();
      render(<DropConfirm landing="comics" summary={BOOK} files={FILES} destinations={INTO} onChoose={onChoose} />);
      if (box === 'Number the pages') {
        // Ticked, so that toggling it twice comes back to a preview to edit.
        fireEvent.click(screen.getByLabelText('Number the volume folders'));
      }
      fireEvent.click(screen.getByLabelText('Number the pages'));
      correct('001.jpg', 'one.jpg');
      // And one left open, which goes with them.
      fireEvent.click(screen.getByRole('button', { name: '002.jpg' }));

      fireEvent.click(screen.getByLabelText(box));
      fireEvent.click(screen.getByLabelText(box));
      expect(screen.queryByRole('textbox')).toBeNull();
      expect(preview()).not.toContain('(edited)');
      expect(preview()).toContain('king01-#001.JPG → 001.jpg');
      fireEvent.click(add());
      expect(onChoose.mock.calls[0][2]).toEqual(NO_EDITS);
    },
  );

  // A corrected volume name sends files into a folder no numbering would, and
  // what the Library holds there is asked for and said as any other.
  it('says what replaces a file the Library holds under a corrected volume name', async () => {
    const file = (path: string) => addedFile(path, { size: 10, lastModified: 0 } as File);
    const files = [file('BookX/vol1/p.jpg'), file('BookX/vol2/p.jpg')];
    const holding = vi.fn(async (folders: readonly string[]): Promise<HeldFiles> =>
      folders.includes('BookX/first') ? new Map([['BookX/first/p.jpg', 'one-file']]) : NOTHING_HELD,
    );
    const onChoose = vi.fn();
    render(
      <DropConfirm landing="comics" summary={BOOK} files={files} destinations={INTO} holding={holding} onChoose={onChoose} />,
    );
    fireEvent.click(screen.getByLabelText('Number the volume folders'));
    expect(holding).not.toHaveBeenCalled();

    correct('001', 'first');
    expect(preview()).toContain('vol1 → first (edited)');
    // Add waits for what the Library holds there, which it would otherwise
    // send without saying.
    expect(add()).toHaveProperty('disabled', true);
    // Said beside Add too, where a long preview may have scrolled the line
    // above away: why a press of Add does nothing yet.
    expect(add().parentElement?.textContent).toContain(
      'reading what the Library holds under the corrected folder names…',
    );
    await waitFor(() => expect(add()).toHaveProperty('disabled', false));
    // And then what the read turned up, above and beside Add.
    const line =
      '1 file being added replaces the file the Library already has at the same path: BookX/first/p.jpg';
    expect(screen.getAllByText(line)).toHaveLength(2);
    expect(add().parentElement?.textContent).toContain(line);
    expect(add().parentElement?.textContent).not.toContain('reading what the Library holds');
    expect(holding).toHaveBeenCalledWith(expect.arrayContaining(['BookX', 'BookX/first']));

    fireEvent.click(add());
    expect(onChoose).toHaveBeenCalledWith(
      'pack',
      { pages: false, volumes: true },
      new Map([['BookX/vol1', 'first']]),
    );
  });
});

// A drop of tens of thousands of files — a series of a hundred volumes added
// at once — opens at once: the incoming items and the preview draw a bounded
// part of what the drop holds and count the rest, while the totals above them
// count every file and every rename.
describe('a drop of ten thousand files', () => {
  const volumes = 100;
  const pages = 100;
  const files: Added[] = Array.from({ length: volumes * pages }, (_, at) =>
    addedFile(`Series/vol ${Math.floor(at / pages) + 1}/p${(at % pages) + 1}.jpg`, {
      size: 1000,
      lastModified: 0,
    } as File),
  );
  const summary = summarize(files, []);
  const into = destinationsOf('comics', summary, (path) => path === 'comics');

  /** The lines the preview draws, folded runs included. */
  function linesDrawn(): number {
    return screen.getByRole('region', { name: 'what is renamed' }).querySelectorAll('li').length;
  }

  it('counts every file and every rename while drawing a bounded part of them', () => {
    window.localStorage.setItem('coffret.drop.number-pages', 'true');
    window.localStorage.setItem('coffret.drop.number-volumes', 'true');
    render(
      <DropConfirm landing="comics" summary={summary} files={files} destinations={into} onChoose={() => undefined} />,
    );
    const said = screen.getByRole('dialog', { name: 'Add to comics' });
    expect(said.textContent).toContain('10000 files, 10.0 MB');
    expect(said.textContent).toContain('📁 Series — 10000 files, 10.0 MB');
    // Every page and every volume renamed, and nothing irregular among them.
    expect(said.textContent).toContain('10100 renamed · 0 to check');

    // Folded, a hundred and one groups still come to more rows than are drawn.
    expect(linesDrawn()).toBeLessThanOrEqual(LINES_DRAWN);
    expect(said.textContent).toMatch(/… and \d+ more lines not drawn/);

    // Unfolded, every line is one to draw, and no more than the bound are.
    fireEvent.click(screen.getByText('Show all'));
    const first = linesDrawn();
    expect(first).toBeGreaterThan(0);
    expect(first).toBeLessThanOrEqual(LINES_DRAWN);
    expect(said.textContent).toContain(`… and ${10100 - first} more lines not drawn`);

    // Each press draws the next part, bounded the same way.
    fireEvent.click(screen.getByRole('button', { name: `Show ${LINES_DRAWN} more` }));
    const second = linesDrawn();
    expect(second).toBeGreaterThan(first);
    expect(second).toBeLessThanOrEqual(2 * LINES_DRAWN);
    expect(said.textContent).toContain(`… and ${10100 - second} more lines not drawn`);
    // The tally still counts all of them.
    expect(said.textContent).toContain('10100 renamed · 0 to check');

    // The incoming items stay one folder, however much it holds.
    expect(screen.getByRole('list', { name: 'incoming' }).querySelectorAll('li')).toHaveLength(1);
  });

  it('names a bounded part of ten thousand loose files and counts the rest', () => {
    const loose = files.map((added, at) => ({ ...added, path: `photo ${at}.jpg` }));
    const looseSummary = summarize(loose, []);
    render(
      <DropConfirm
        landing="comics"
        summary={looseSummary}
        files={loose}
        destinations={destinationsOf('comics', looseSummary, (path) => path === 'comics')}
        onChoose={() => undefined}
      />,
    );
    const said = screen.getByRole('dialog', { name: 'Add to comics' });
    expect(said.textContent).toContain('10000 files, 10.0 MB');
    const items = screen.getByRole('list', { name: 'incoming' }).querySelectorAll('li');
    expect(items.length).toBeLessThanOrEqual(10);
    expect(items[items.length - 1].textContent).toBe(`… and ${10000 - (items.length - 1)} more`);
  });
});
