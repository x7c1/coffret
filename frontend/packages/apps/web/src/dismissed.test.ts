import { expect, it } from 'vitest';

import type { Fill, Sync } from '@coffret/api';

import {
  canPutAway,
  isPutAway,
  NOTHING_DISMISSED,
  putAway,
  putAwayFolders,
  servedBy,
  shownFolders,
  stillOffered,
} from './dismissed';

function syncing(over: Partial<Sync> = {}): Sync {
  return {
    run: 1,
    status: 'done',
    added: 1,
    findings: [
      {
        path: 'books/vol-1/page-001.png',
        message: 'it is inside a Pack',
        reason: 'surfaced',
        surfaced: 'ChangedInPack',
      },
    ],
    step: null,
    stopped: null,
    ...over,
  };
}

function filling(over: Partial<Fill> = {}): Fill {
  return {
    run: 1,
    folder: 'albums',
    status: 'filling',
    total: 3,
    done: 1,
    declined: [],
    waiting: [],
    dropped: [],
    displaced: [],
    stopped: null,
    ...over,
  };
}

// The whole of what this buys: the line a finished sync leaves has nothing to
// end it, and every later fill's progress is drawn underneath it until the tab
// is closed. Reading it is what ends it.
it('puts away the line of the run it was pressed over', () => {
  const read = putAway(NOTHING_DISMISSED, 'sync', 1);

  expect(isPutAway(read, 'sync', syncing())).toBe(true);
  expect(isPutAway(NOTHING_DISMISSED, 'sync', syncing())).toBe(false);
});

// What is put away is a run and not a sentence. Two runs of one flow can come
// to exactly the same words, and swallowing the second one's — a finding saying
// a dropped file is still not backed up — would be the whole loss.
it('says nothing about the next run of the same flow', () => {
  const read = putAway(NOTHING_DISMISSED, 'sync', 1);

  expect(isPutAway(read, 'sync', syncing({ run: 2 }))).toBe(false);
});

// An answer that was already in flight when the button was pressed is about a
// run somebody has finished with, and bringing its line back for one tick would
// be a dismissal that flickers.
it('stays put away for an answer from before it was pressed', () => {
  const read = putAway(NOTHING_DISMISSED, 'fill', 4);

  expect(isPutAway(read, 'fill', filling({ run: 3, status: 'done' }))).toBe(true);
});

// One flow's line is not another's: a sync somebody has read says nothing about
// the fill running underneath it.
it('keeps the three flows apart', () => {
  const read = putAway(NOTHING_DISMISSED, 'sync', 1);

  expect(isPutAway(read, 'fill', filling({ status: 'done' }))).toBe(false);
  expect(isPutAway(read, 'freeze', null)).toBe(false);
});

// A line about work happening now is not something a person has finished with:
// its counts are still moving, and the next answer would bring it straight
// back.
it('offers no dismissal for a run that is still going', () => {
  expect(canPutAway(filling())).toBe(false);
  expect(canPutAway(syncing({ status: 'syncing' }))).toBe(false);
  expect(canPutAway(null)).toBe(false);

  expect(canPutAway(syncing())).toBe(true);
  expect(canPutAway(filling({ status: 'stopped' }))).toBe(true);
  // A superseded fill is over in the same sense: nothing takes it up again on
  // its own, so its line is as final as a finished one's.
  expect(canPutAway(filling({ status: 'superseded' }))).toBe(true);
});

// The one kind of notice that had no way out. A folder a worker threw away is
// not a run, so there was no number to put away and no button offered — and the
// server keeps the offer until that folder is armed, which for somebody who has
// decided against it is its line and its button for the life of the process.
it('puts away the folders a queue lost, by name', () => {
  const read = putAwayFolders(NOTHING_DISMISSED, 'fill', ['books', 'letters']);

  expect(shownFolders(read, 'fill', ['books', 'letters'])).toEqual([]);
  expect(shownFolders(NOTHING_DISMISSED, 'fill', ['books'])).toEqual(['books']);
});

// One queue's losses are not the other's: a book the freeze lost says nothing
// about a folder of the same name the fill lost.
it('keeps the two queues of lost folders apart', () => {
  const read = putAwayFolders(NOTHING_DISMISSED, 'freeze', ['books']);

  expect(shownFolders(read, 'fill', ['books'])).toEqual(['books']);
});

// What ends the dismissal, and what a run number's being spent by the next run
// corresponds to here. The offer leaving the server's list is the folder having
// been taken up — by this button or by somebody opening a file in it — so a
// folder thrown away a second time arrives as an offer nobody has answered.
it('lets a folder dropped a second time come back', () => {
  const read = putAwayFolders(NOTHING_DISMISSED, 'fill', ['books']);
  expect(shownFolders(read, 'fill', ['books'])).toEqual([]);

  // Armed: the server stops offering it, and that is where it is forgotten.
  const taken = stillOffered(read, 'fill', []);

  // And thrown away again, by a later worker that ended without an answer.
  expect(shownFolders(taken, 'fill', ['books'])).toEqual(['books']);
});

// While the offer stands it stays away, however many answers carry it: a
// dismissal that lasted one tick would be no dismissal at all.
it('keeps a folder away for as long as it is still offered', () => {
  const read = putAwayFolders(NOTHING_DISMISSED, 'fill', ['books', 'letters']);
  const still = stillOffered(stillOffered(read, 'fill', ['books', 'letters']), 'fill', [
    'books',
  ]);

  expect(shownFolders(still, 'fill', ['books'])).toEqual([]);
  expect(
    shownFolders(still, 'fill', ['books', 'letters']),
    'the one the server stopped offering is forgotten with it',
  ).toEqual(['letters']);
});

// The two kinds of dismissal are kept in one value and do not reach each other:
// a line put away says nothing about the folders that queue lost, which is the
// whole of why they are on the bar at all.
it('says nothing about the runs when a lost folder is put away', () => {
  const read = putAwayFolders(putAway(NOTHING_DISMISSED, 'fill', 2), 'fill', ['books']);

  expect(isPutAway(read, 'fill', filling({ run: 2, status: 'done' }))).toBe(true);
  expect(shownFolders(read, 'fill', ['books'])).toEqual([]);
  expect(shownFolders(putAway(NOTHING_DISMISSED, 'fill', 2), 'fill', ['books'])).toEqual([
    'books',
  ]);
});

// The thing no run number can say. A run counts from 1 with each process, and a
// locked Library is opened by typing the Passphrase and starting the server
// again — so a tab left open across a restart would go on hiding the new
// process's first runs: a fill's line and its chips, and the one sentence that
// says a sync did not back a file up. The name of the process is what tells the
// two apart.
it('forgets everything once the answers come from another process', () => {
  const held = putAwayFolders(
    putAway(servedBy(NOTHING_DISMISSED, 'a1b2'), 'sync', 2),
    'fill',
    ['books'],
  );

  const restarted = servedBy(held, 'c3d4');

  expect(isPutAway(restarted, 'sync', syncing({ run: 2 }))).toBe(false);
  expect(
    isPutAway(restarted, 'sync', syncing({ run: 1 })),
    'the new process counts from 1, and none of its runs has been read',
  ).toBe(false);
  expect(shownFolders(restarted, 'fill', ['books'])).toEqual(['books']);
});

// And the case it must not be confused with: an answer from the process the
// dismissal was made under, carrying a run older than the one put away, is a
// request that was already in flight when the button was pressed. Its line
// stays away — bringing it back for one tick is a dismissal that flickers.
it('leaves a late answer from the same process put away', () => {
  const held = putAway(servedBy(NOTHING_DISMISSED, 'a1b2'), 'sync', 2);

  const same = servedBy(held, 'a1b2');

  expect(isPutAway(same, 'sync', syncing({ run: 1 }))).toBe(true);
  expect(isPutAway(same, 'sync', syncing({ run: 2 }))).toBe(true);
  expect(isPutAway(same, 'sync', syncing({ run: 3 }))).toBe(false);
});

// The first answer a tab hears is a change too, and there is nothing to lose to
// it: a run can only be put away after an answer named it.
it('takes the name of the first process it hears from', () => {
  expect(servedBy(NOTHING_DISMISSED, 'a1b2').server).toBe('a1b2');
  expect(isPutAway(servedBy(NOTHING_DISMISSED, 'a1b2'), 'sync', syncing())).toBe(false);
});
