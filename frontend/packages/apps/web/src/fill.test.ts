import { expect, it } from 'vitest';

import type { Catalog, Fill, Freeze, ListedFile, Sync } from '@coffret/api';

import {
  addingLine,
  collectingLine,
  droppedBooksLine,
  droppedLine,
  fillLine,
  fillOfFolder,
  freezeLine,
  freezingHere,
  isFreezing,
  rowFill,
  shouldAsk,
  shouldPoll,
  stoppedBooksLine,
  stoppedLine,
  syncLine,
} from './fill';

function caught(over: Partial<Catalog> = {}): Catalog {
  return { state: 'caught_up', trouble: null, ...over };
}

function file(path: string, state: ListedFile['state']): ListedFile {
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

function syncing(over: Partial<Sync> = {}): Sync {
  return {
    run: 1,
    step: null,
    status: 'syncing',
    added: 0,
    noted: [],
    stopped: null,
    ...over,
  };
}

function freezing(over: Partial<Freeze> = {}): Freeze {
  return {
    run: 1,
    step: null,
    waiting: [],
    dropped: [],
    displaced: [],
    folder: 'books/vol-1',
    status: 'freezing',
    packs: 0,
    entries: 0,
    noted: [],
    stopped: null,
    ...over,
  };
}

function filling(over: Partial<Fill> = {}): Fill {
  return {
    run: 1,
    waiting: [],
    dropped: [],
    displaced: [],
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

// EP-13: the mapped root was refused, so the fill placed nothing. The refusal
// carries recovery guidance now, and the line the status bar already writes is
// what puts it in front of a person — so this needs no case in the explorer
// and no new state on a row. The existing line is asked to prove it says the
// whole thing.
it('shows what a refused mapped root says, on the line a stopped fill already has', () => {
  const refused =
    'the folder this device maps "albums" into is not the folder that mapping was recorded ' +
    'against, so nothing was put into it. Open a terminal on the device serving the Library. Run ' +
    '`coffret mappings --library <library>` to inspect the recorded mappings and `coffret map ' +
    '--help` to find the arguments. Use that listing to choose one recovery: reconnect the intended ' +
    'folder if it is elsewhere; if the folder at the recorded location is the intended one, record ' +
    'this mapping again with `coffret map`; or map another folder in its place only as a deliberate ' +
    'choice. If `coffret map` reports a local marker problem, correct the problem and run it again. ' +
    'Then return to the explorer and try the action again';
  const stopped = filling({
    folder: 'albums',
    status: 'stopped',
    done: 0,
    stopped: { error: 'declined', message: refused, reason: 'refused_root' },
  });

  expect(fillLine(stopped)).toBe(`could not bring over albums — ${refused}`);
  // And a row it never reached says the same thing rather than a shrug: what
  // stopped the fill is what would have stopped this file.
  expect(rowFill(file('albums/notes.txt', 'remote'), 'albums', stopped)).toEqual({
    state: 'failed',
    message: refused,
  });
});

// An idle explorer issues no requests at all: the interval exists for the
// minutes a fill takes, not for a tab left open on a folder.
it('polls while the reader is open or work is running, and not otherwise', () => {
  expect(shouldPoll(false, null, null)).toBe(false);
  expect(shouldPoll(true, null, null)).toBe(true);
  expect(shouldPoll(false, filling(), null)).toBe(true);
  expect(shouldPoll(false, filling({ status: 'done' }), null)).toBe(false);
  expect(shouldPoll(false, filling({ status: 'stopped' }), null)).toBe(false);
  expect(shouldPoll(false, filling({ status: 'superseded' }), null)).toBe(false);
});

// The reload case, stated on its own because it is the one that goes wrong
// quietly: the reader is shut, the fill is the server's and is still running,
// and the page that just came up has forgotten it. What brings it back is the
// one question a page asks as it comes up, and then this — a fill under way is
// a reason to keep asking whatever the reader is doing.
it('follows a fill through a reload, with the reader shut', () => {
  expect(shouldAsk(false, false)).toBe(true);
  expect(shouldPoll(false, filling(), null)).toBe(true);
  // Armed and not yet counted is still running.
  expect(shouldPoll(false, filling({ total: 0, done: 0 }), null)).toBe(true);
});

// A catch-up is every folder's answer being about to change at once, which is
// as much a reason to keep asking as a fill is — and a page that came up while
// one was running would otherwise sit on "catching up" for as long as the tab
// was open.
it('polls while this device is catching up with the Library', () => {
  expect(shouldPoll(false, null, null, null, caught({ state: 'catching_up' }))).toBe(true);
  expect(shouldPoll(false, null, null, null, caught())).toBe(false);
  // Nothing is running, so there is nothing to follow: what moves a catalog
  // that is behind is the control that asks again.
  expect(shouldPoll(false, null, null, null, caught({ state: 'behind' }))).toBe(false);
});

// A book waiting its turn is work in flight even where the one on record has
// finished: the worker takes the next one, and a page that stopped asking would
// miss the whole of it.
it('polls while a book is waiting its turn', () => {
  expect(
    shouldPoll(false, null, null, freezing({ status: 'done', waiting: ['books/vol-2'] })),
  ).toBe(true);
  expect(shouldPoll(false, null, null, freezing({ status: 'done' }))).toBe(false);
});

// And the same for a folder asked for by name: it is a press the server has
// taken and not answered for yet, so a page that stopped asking would leave it
// with no ending on the screen.
it('polls while a folder is waiting its turn', () => {
  expect(shouldPoll(false, filling({ status: 'done', waiting: ['letters'] }), null)).toBe(true);
  expect(shouldPoll(false, filling({ status: 'done' }), null)).toBe(false);
});

// A sync is the rows of the folder somebody just dropped into being about to
// change, so it is followed for the same reason a fill is — and the reader has
// nothing to do with it.
it('polls while a sync is running, whatever the reader is doing', () => {
  expect(shouldPoll(false, null, syncing())).toBe(true);
  expect(shouldPoll(false, null, syncing({ status: 'done' }))).toBe(false);
  expect(shouldPoll(false, null, syncing({ status: 'stopped' }))).toBe(false);
});

// A file in the folder that the Library does not have yet. No fill is about it —
// there is no Entry to fetch — and the chip says what it is rather than leaving
// it looking like a row that failed.
it('marks a file the Library does not hold yet', () => {
  const shown = rowFill(file('albums/dropped.jpg', 'uploading'), 'albums', null);
  expect(shown.state).toBe('uploading');
  expect(shown.message).not.toBeNull();

  expect(rowFill(file('albums/dropped.jpg', 'uploading'), 'albums', filling()).state).toBe(
    'uploading',
  );
});

// A fill that finished having declined something and one that stopped both end
// at "28/30", and they are opposite answers: one is over and the other is not.
// So the finished one says so rather than taking its line off the screen.
it('tells a fill that declined something from a fill that stopped', () => {
  const declined = filling({
    status: 'done',
    total: 3,
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
  expect(fillLine(declined)).toBe(
    'brought over 2/3 in books/vol-1 — 1 file was not placed: ' +
      'books/vol-1/page-003.png — a file this device did not put there stands where ' +
      'this Entry belongs',
  );

  const stopped = filling({
    status: 'stopped',
    total: 3,
    done: 2,
    stopped: { error: 'storage', message: 'Storage did not answer' },
  });
  expect(fillLine(stopped)).toBe('could not bring over books/vol-1 — Storage did not answer');
  expect(fillLine(declined)).not.toBe(fillLine(stopped));
});

// One line for all of them, because a folder of three hundred Entries declined
// for the one reason would otherwise be three hundred sentences in a bar one
// line high. The rows carry the rest, each marked with its own.
it('names the first declined Entry and counts the others', () => {
  expect(
    fillLine(
      filling({
        status: 'done',
        total: 4,
        done: 2,
        declined: [
          { path: 'a.jpg', error: 'declined', message: 'one' },
          { path: 'b.jpg', error: 'declined', message: 'two' },
        ],
      }),
    ),
  ).toBe('brought over 2/4 in books/vol-1 — 2 files were not placed: a.jpg — one (and 1 more)');
});

// The folders a worker that ended without an answer threw away. The fill's own
// line and its retry both name the folder that died, so without this a person
// takes that one up again and never learns the rest went with it.
it('names the folders the queue lost when a worker left', () => {
  expect(droppedLine([])).toBeNull();
  expect(droppedLine(['books/vol-2'])).toBe(
    'books/vol-2 was dropped before it was brought over',
  );
  expect(droppedLine(['books/vol-2', 'albums'])).toBe(
    'books/vol-2 and 1 more were dropped before they were brought over',
  );
  // The Library root has no name of its own here either.
  expect(droppedLine([''])).toBe('the Library root was dropped before it was brought over');
});

// The freeze's queue loses books the same way, and says so in its own words: a
// fill worker and a freeze worker are separate tasks and can lose folders at
// the same moment, so a person owed both sentences must not be given one twice.
it('names the books the freeze queue lost in the freeze words', () => {
  expect(droppedBooksLine([])).toBeNull();
  expect(droppedBooksLine(['books/vol-2'])).toBe(
    'books/vol-2 was dropped before it was packed',
  );
  expect(droppedBooksLine(['books/vol-2'])).not.toBe(droppedLine(['books/vol-2']));
});

// A run Storage stopped that the next folder took the record from. It says what
// it said while it was the run on record — nothing about it changed when the
// next one started — and it says it through the same function, so the sentence
// cannot drift from the one the bar showed a tick earlier.
it('keeps the sentence of a run the next one took the record from', () => {
  expect(stoppedLine([])).toBeNull();
  const stopped = filling({
    folder: 'albums',
    status: 'stopped',
    stopped: { error: 'storage', message: 'Storage did not answer' },
  });
  expect(stoppedLine([stopped])).toBe(fillLine(stopped));
  expect(stoppedLine([stopped])).toBe('could not bring over albums — Storage did not answer');
});

// One Storage outage stops every folder queued behind the first, and the bar has
// room for one line: the oldest speaks, and the count says how many stand behind
// it — each with a button of its own naming which.
it('counts the runs standing behind the one it names', () => {
  const first = filling({ folder: 'albums', status: 'stopped' });
  const second = filling({ folder: 'letters', status: 'stopped' });

  expect(stoppedLine([first, second])).toBe(`${fillLine(first)} (and 1 more stopped)`);
});

// The books say it in the freeze's own words, for the reason the dropped lines
// do: the two flows are separate tasks and can stop at the same moment, so a
// person owed both sentences must not be given one of them twice.
it('keeps the sentence of a book the next one took the record from', () => {
  expect(stoppedBooksLine([])).toBeNull();
  const stopped = freezing({
    folder: 'books/vol-1',
    status: 'stopped',
    stopped: { error: 'storage', message: 'Storage did not answer' },
  });

  expect(stoppedBooksLine([stopped])).toBe('could not pack books/vol-1 — Storage did not answer');
  expect(stoppedBooksLine([stopped])).not.toBe(stoppedLine([filling({ status: 'stopped' })]));
});

// Which run the rows of a folder read. The one on record where it is about this
// folder, and otherwise the run that stopped on it — the `failed` and `declined`
// chips are the other half of what that run's line says, and are owed for as
// long as it is.
it('gives the rows the run that is about their folder', () => {
  const running = filling({ folder: 'letters', status: 'filling' });
  const stopped = filling({ folder: 'albums', status: 'stopped' });

  expect(fillOfFolder(running, [stopped], 'letters')).toBe(running);
  expect(fillOfFolder(running, [stopped], 'albums')).toBe(stopped);
  expect(fillOfFolder(running, [stopped], 'books')).toBeNull();
  // A run somebody has put away is not among the ones handed in, so the rows
  // fall back to what the listing says — as they do when the fill's own line is
  // put away.
  expect(fillOfFolder(null, [], 'albums')).toBeNull();
});

it('says what a sync is doing, and says nothing once it is over', () => {
  expect(syncLine(syncing())).toBe('backing up what was added…');
  expect(syncLine(null)).toBeNull();
  expect(syncLine(syncing({ status: 'done', added: 2 }))).toBeNull();
});

// PK-14: a run that returns Ok has not necessarily backed everything up, and
// the person who dropped the file is not at a terminal to be told so. The line
// is the only place they hear it.
it('keeps a line for a sync that left something alone', () => {
  expect(
    syncLine(
      syncing({
        status: 'done',
        added: 1,
        noted: [{ path: 'books/vol-1/page-001.png', message: 'it is inside a Pack' }],
      }),
    ),
  ).toBe('books/vol-1/page-001.png — it is inside a Pack');

  expect(
    syncLine(
      syncing({
        status: 'done',
        noted: [
          { path: 'a.jpg', message: 'one' },
          { path: 'b.jpg', message: 'two' },
        ],
      }),
    ),
  ).toBe('a.jpg — one (and 1 more)');

  // A finding about no single Entry has no path to name, and reads as the
  // sentence alone rather than as one about a file called `null`.
  expect(
    syncLine(syncing({ status: 'done', noted: [{ path: null, message: 'a folder went' }] })),
  ).toBe('a folder went');
});

it('keeps a line for a sync that stopped, because the retry hangs off it', () => {
  expect(
    syncLine(
      syncing({
        status: 'stopped',
        stopped: { error: 'storage', message: "the Library's Storage did not answer" },
      }),
    ),
  ).toBe("could not back up what was added — the Library's Storage did not answer");
});

// A book is followed for the same reason a sync is, and for longer: the pages
// of a whole folder are about to become rows. The reader has nothing to do
// with it.
it('polls while a book is being packed, whatever the reader is doing', () => {
  expect(shouldPoll(false, null, null, freezing())).toBe(true);
  expect(shouldPoll(false, null, null, freezing({ status: 'done' }))).toBe(false);
  expect(shouldPoll(false, null, null, freezing({ status: 'stopped' }))).toBe(false);
  expect(shouldPoll(false, null, null, null)).toBe(false);
});

// One book at a time: what the screen reads to know not to offer a second.
it('says whether a book is being packed right now', () => {
  expect(isFreezing(freezing())).toBe(true);
  expect(isFreezing(freezing({ status: 'done' }))).toBe(false);
  expect(isFreezing(null)).toBe(false);
});

// A freeze of somewhere else is somebody else's book, and this folder is what
// the listing says it is — the rule a fill's rows already follow.
it('marks only the folder being packed', () => {
  expect(freezingHere(freezing(), 'books/vol-1')).toBe(true);
  expect(freezingHere(freezing(), 'books/vol-2')).toBe(false);
  expect(freezingHere(freezing({ status: 'done' }), 'books/vol-1')).toBe(false);
  expect(freezingHere(null, 'books/vol-1')).toBe(false);
});

// Unlike a finished fill or a quiet sync, a finished freeze says what it came
// to: that the several hundred pages went up as a handful of objects is the
// whole of what the person dropping a book was after, and no row can say it.
it('says what a book came to, and stays while it is being packed', () => {
  expect(freezeLine(null)).toBeNull();
  expect(freezeLine(freezing())).toBe('packing books/vol-1…');
  expect(freezeLine(freezing({ folder: '' }))).toBe('packing the Library root…');
  expect(freezeLine(freezing({ status: 'done', packs: 1, entries: 240 }))).toBe(
    'packed 240 files of books/vol-1 into 1 Pack',
  );
  expect(freezeLine(freezing({ status: 'done', packs: 3, entries: 2 }))).toBe(
    'packed 2 files of books/vol-1 into 3 Packs',
  );
  // A second run over a folder every file of which is already in a Pack has
  // nothing to do, and saying "packed 0 files into 0 Packs" would read as a
  // failure rather than as the ordinary answer.
  expect(freezeLine(freezing({ status: 'done' }))).toBe(
    'books/vol-1 was already packed',
  );
});

// PK-14: a run that returns Ok has not necessarily packed everything, and the
// person who dropped the book is not at a terminal to be told so.
it('keeps a line for a freeze that left a page alone', () => {
  expect(
    freezeLine(
      freezing({
        status: 'done',
        packs: 1,
        entries: 2,
        noted: [{ path: 'books/vol-1/page-003.jpg', message: 'it is inside a Pack' }],
      }),
    ),
  ).toBe('books/vol-1/page-003.jpg — it is inside a Pack');
});

it('keeps a line for a freeze that stopped, because the retry hangs off it', () => {
  expect(
    freezeLine(
      freezing({
        status: 'stopped',
        stopped: { error: 'storage', message: "the Library's Storage did not answer" },
      }),
    ),
  ).toBe("could not pack books/vol-1 — the Library's Storage did not answer");
});

// The page comes up and asks once, whether or not anything is running: "nothing
// in flight" is a statement about this page and not about the server, and a run
// Storage stopped is still stopped after a reload.
it('asks once as the page comes up, and not again by itself', () => {
  expect(shouldAsk(false, false)).toBe(true);
  expect(shouldAsk(true, false)).toBe(false);
});

// A question that never answered taught this page nothing, so it does not count
// as having asked: a tab whose one question at the start failed would otherwise
// never again say what the server is doing — and nothing about that failure is
// on the screen to press about.
it('asks again after a question nothing answered', () => {
  const told = false;
  expect(shouldAsk(told, false)).toBe(true);
});

// And the interval's own reason is untouched: while there is something to
// follow, every tick is a reason to ask again.
it('keeps asking while there is something to follow', () => {
  expect(shouldAsk(false, true)).toBe(true);
  expect(shouldAsk(true, true)).toBe(true);
});

// A quiet server answers once and is left alone. Nothing running is nothing to
// poll, so the one question at the start is the whole of what an explorer that
// came up to a finished Library asks — the idle discipline, kept.
it('asks a quiet server once and then nothing at all', () => {
  const quiet = shouldPoll(
    false,
    filling({ status: 'done', done: 3 }),
    syncing({ status: 'done' }),
    freezing({ status: 'done', packs: 1, entries: 2 }),
  );
  expect(quiet).toBe(false);
  expect(shouldAsk(true, quiet)).toBe(false);
});

// The walkthrough a reload used to lose. The page comes up, asks its one
// question, and the answer is a book Storage stopped packing: the line the
// "pack again" hangs off is back on the bar — and no interval starts behind it,
// because the run is over and there is nothing to follow.
it('comes back from a reload with the stopped book on the bar and nothing polling', () => {
  const stopped = freezing({
    status: 'stopped',
    stopped: { error: 'storage', message: "the Library's Storage did not answer" },
  });

  expect(shouldAsk(false, shouldPoll(false, null, null, null))).toBe(true);
  expect(freezeLine(stopped)).toBe(
    "could not pack books/vol-1 — the Library's Storage did not answer",
  );
  expect(shouldAsk(true, shouldPoll(false, null, null, stopped))).toBe(false);
});

it('counts the files a drop is still sending', () => {
  expect(addingLine(1, 'albums/2026')).toBe('adding 1 file to albums/2026…');
  expect(addingLine(3, 'albums/2026')).toBe('adding 3 files to albums/2026…');
  expect(addingLine(2, '')).toBe('adding 2 files to the Library root…');
});

// The flow's own answer about where it has got to, rendered rather than
// invented: it is the same step the command line draws its line from, so a
// browser and a terminal watching one run cannot disagree about it.
it('says which phase a run is in and how far into it it is', () => {
  expect(syncLine(syncing({ step: { phase: 'packing', done: 3, total: 10 } }))).toBe(
    'backing up what was added — packing 3/10…',
  );
  expect(
    freezeLine(freezing({ step: { phase: 'uploading', done: 1, total: 4 } })),
  ).toBe('packing books/vol-1 — sending 1/4…');
});

// And it says it once. The freeze's own line opens with the word its `packing`
// phase goes under, and a clause that repeated it would read "packing
// books/vol-1 — packing 12/300": the same word twice, with the only new thing
// in the clause hidden behind it.
it('says what a freeze is doing once rather than twice', () => {
  expect(freezeLine(freezing({ step: { phase: 'packing', done: 12, total: 300 } }))).toBe(
    'packing books/vol-1 — 12/300…',
  );
  expect(
    freezeLine(
      freezing({ step: { phase: 'packing', done: 12, total: 300 }, waiting: ['books/vol-2'] }),
    ),
  ).toBe('packing books/vol-1 — 12/300, with books/vol-2 after it…');
  // And a packing phase that cannot count its work says nothing rather than the
  // word a second time.
  expect(freezeLine(freezing({ step: { phase: 'packing', done: 0, total: null } }))).toBe(
    'packing books/vol-1…',
  );
});

// A phase that cannot count its work says its name and no numbers. It is not
// the same state as a phase with nothing in it, and a `0/0` beside it would
// read as work already done — while that phase is exactly the one that goes
// quiet for minutes.
it('shows a phase that cannot count its work without numbers', () => {
  expect(syncLine(syncing({ step: { phase: 'catching_up', done: 0, total: null } }))).toBe(
    'backing up what was added — catching up with the Library…',
  );
});

// A person who dropped a second book wants to know that theirs is queued, which
// a count of one cannot tell them: the line names it.
it('names the books waiting behind the one being packed', () => {
  expect(freezeLine(freezing({ waiting: ['books/vol-2'] }))).toBe(
    'packing books/vol-1, with books/vol-2 after it…',
  );
  expect(freezeLine(freezing({ waiting: ['books/vol-2', 'books/vol-3'] }))).toBe(
    'packing books/vol-1, with books/vol-2 and 1 more after it…',
  );
});

// And the same for a folder somebody asked for by name while a fill was
// running. The press takes the button that named it away and the line names the
// folder being brought over, so without this the press leaves no trace at all —
// which is the whole of what queueing rather than displacing was for.
it('names the folders waiting behind the one being brought over', () => {
  expect(fillLine(filling({ waiting: ['letters'] }))).toBe(
    'bringing over 1/3 in books/vol-1, with letters after it…',
  );
  expect(fillLine(filling({ total: 0, waiting: ['letters', 'albums'] }))).toBe(
    'bringing over books/vol-1, with letters and 1 more after it…',
  );
  expect(fillLine(filling())).toBe('bringing over 1/3 in books/vol-1…');
});

// The seconds between a drop being let go of and there being anything to send.
// A nested folder is walked one batch of children at a time, and its files land
// one folder down — so the folder on the screen gains no row to say they are
// there, and without this the gesture is answered by nothing at all.
it('says something from the moment a drop is taken', () => {
  expect(collectingLine()).not.toBe('');
  expect(collectingLine()).not.toBe(addingLine(1, 'albums'));
});
