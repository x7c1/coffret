import { expect, it } from 'vitest';

import type { Fill, ListedFile } from '@coffret/api';

import { fillLine, rowFill, shouldPoll } from './fill';

function file(path: string, state: 'present' | 'remote'): ListedFile {
  return {
    name: path.split('/').at(-1) ?? path,
    path,
    size: 1,
    mtime: null,
    state,
    container: 'pack',
    openable: true,
    content_type: 'image/jpeg',
  };
}

function filling(over: Partial<Fill> = {}): Fill {
  return {
    folder: 'books/vol-1',
    status: 'filling',
    total: 3,
    done: 1,
    declined: [],
    stopped: null,
    ...over,
  };
}

it('shows what the listing says when nothing is being filled', () => {
  expect(rowFill(file('books/vol-1/page-002.png', 'remote'), 'books/vol-1', null)).toEqual({
    state: 'remote',
    message: null,
  });
  expect(rowFill(file('books/vol-1/page-001.png', 'present'), 'books/vol-1', null)).toEqual({
    state: 'present',
    message: null,
  });
});

it('marks the remote rows of the folder being filled', () => {
  expect(rowFill(file('books/vol-1/page-002.png', 'remote'), 'books/vol-1', filling())).toEqual({
    state: 'fetching',
    message: null,
  });
});

// The listing is the one answer about what is on this device: a row it calls
// present is present, whatever a fill a moment behind it still says.
it('never contradicts a listing that says a row is present', () => {
  expect(rowFill(file('books/vol-1/page-001.png', 'present'), 'books/vol-1', filling())).toEqual({
    state: 'present',
    message: null,
  });
});

// A fill of somewhere else is somebody else's folder being brought over.
it('says nothing about the rows of a folder it is not filling', () => {
  expect(rowFill(file('books/vol-2/page-001.png', 'remote'), 'books/vol-2', filling())).toEqual({
    state: 'remote',
    message: null,
  });
});

// A declined Entry is a finding about that one Entry and not a failure: the row
// shows what opening it would have said, without anybody opening it.
it('marks a declined Entry with what the file route would have said', () => {
  const fill = filling({
    status: 'done',
    done: 2,
    declined: [
      {
        path: 'books/vol-1/page-003.png',
        error: 'declined',
        message: 'a file this device did not put there stands where this Entry belongs',
        reason: 'surfaced',
        surfaced: 'ForeignFile',
      },
    ],
  });

  expect(rowFill(file('books/vol-1/page-003.png', 'remote'), 'books/vol-1', fill)).toEqual({
    state: 'declined',
    message: 'a file this device did not put there stands where this Entry belongs',
  });
  // And the one it never reached is remote, not failed: the fill finished.
  expect(rowFill(file('books/vol-1/page-004.png', 'remote'), 'books/vol-1', fill)).toEqual({
    state: 'remote',
    message: null,
  });
});

it('marks the rows a fill Storage stopped never reached', () => {
  const stopped = filling({
    status: 'stopped',
    done: 1,
    stopped: { error: 'storage', message: "the Library's Storage did not answer" },
  });

  expect(rowFill(file('books/vol-1/page-002.png', 'remote'), 'books/vol-1', stopped)).toEqual({
    state: 'failed',
    message: "the Library's Storage did not answer",
  });
});

it('counts progress in the status line, and says nothing once it is over', () => {
  expect(fillLine(filling())).toBe('bringing over 1/3 in books/vol-1…');
  expect(fillLine(null)).toBeNull();
  expect(fillLine(filling({ status: 'done', done: 3 }))).toBeNull();
  expect(fillLine(filling({ status: 'superseded' }))).toBeNull();
});

// The total is unknown until the folder's listing has been read, and `0/0`
// would read as nothing to do rather than as not yet known.
it('leaves the counts out until the folder has been counted', () => {
  expect(fillLine(filling({ total: 0, done: 0 }))).toBe('bringing over books/vol-1…');
  expect(fillLine(filling({ total: 0, done: 0, folder: '' }))).toBe(
    'bringing over the Library root…',
  );
});

it('keeps a line for a fill that stopped, because the retry hangs off it', () => {
  expect(
    fillLine(
      filling({
        status: 'stopped',
        stopped: { error: 'storage', message: "the Library's Storage did not answer" },
      }),
    ),
  ).toBe("could not bring over books/vol-1 — the Library's Storage did not answer");
});

// An idle explorer issues no requests at all: the interval exists for the
// minutes a fill takes, not for a tab left open on a folder.
it('polls while the reader is open or a fill is running, and not otherwise', () => {
  expect(shouldPoll(false, null)).toBe(false);
  expect(shouldPoll(true, null)).toBe(true);
  expect(shouldPoll(false, filling())).toBe(true);
  expect(shouldPoll(false, filling({ status: 'done' }))).toBe(false);
  expect(shouldPoll(false, filling({ status: 'stopped' }))).toBe(false);
  expect(shouldPoll(false, filling({ status: 'superseded' }))).toBe(false);
});
