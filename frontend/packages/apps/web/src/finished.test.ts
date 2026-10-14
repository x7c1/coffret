import { expect, it } from 'vitest';

import type { Delete, Freeze, Sync } from '@coffret/api';

import { finishedAfter, latestFirst, NOTHING_FINISHED, type Ended } from './finished';

function sync(run: number, status: 'syncing' | 'done'): Sync {
  return { run, status, stopped: null, added: 1, findings: [], step: null, waits: false };
}

function freeze(run: number, status: 'freezing' | 'done'): Freeze {
  return {
    run,
    status,
    stopped: null,
    folder: 'books/vol-1',
    packs: 1,
    entries: 3,
    findings: [],
    step: null,
    waits: false,
    waiting: [],
    discarded: [],
    displaced: [],
  };
}

function deletion(run: number, status: 'deleting' | 'done'): Delete {
  return {
    run,
    status,
    stopped: null,
    folder: null,
    paths: ['albums/a.jpg'],
    entries: 1,
    bytes: 10,
    removed: 1,
    rebuilt: 0,
    rebuild_read: 0,
    rebuild_written: 0,
    refused: [],
    missing: [],
    findings: [],
    step: null,
    waits: false,
    waiting: 0,
  };
}

function answer(over: Partial<Ended> = {}): Ended {
  return { server: 'a1', sync: null, freeze: null, delete: null, ...over };
}

// A Pack, then one file added one by one, then a deletion: each ending moves
// its flow to the front, so the bar's latest result follows what happened
// last rather than which flow ranks highest.
it('puts each flow first as an answer reports its run finished', () => {
  const packed = finishedAfter(NOTHING_FINISHED, answer({ freeze: freeze(1, 'done') }));
  expect(packed.order).toEqual(['freeze']);

  const adding = finishedAfter(
    packed,
    answer({ freeze: freeze(1, 'done'), sync: sync(1, 'syncing') }),
  );
  expect(adding).toBe(packed);

  const added = finishedAfter(adding, answer({ freeze: freeze(1, 'done'), sync: sync(1, 'done') }));
  expect(added.order).toEqual(['sync', 'freeze']);

  const deleted = finishedAfter(
    added,
    answer({ freeze: freeze(1, 'done'), sync: sync(1, 'done'), delete: deletion(1, 'done') }),
  );
  expect(deleted.order).toEqual(['delete', 'sync', 'freeze']);

  // A later freeze finishing takes the front again; the same run seen again
  // does not.
  const again = finishedAfter(
    deleted,
    answer({ freeze: freeze(2, 'done'), sync: sync(1, 'done'), delete: deletion(1, 'done') }),
  );
  expect(again.order).toEqual(['freeze', 'delete', 'sync']);
  expect(finishedAfter(again, answer({ freeze: freeze(2, 'done') }))).toBe(again);
});

// Endings one answer cannot tell apart take the order the bar ranked flows in
// before, which is also what a page that comes up to three finished runs sees.
it('orders endings reported together as the freeze, the sync, the deletion', () => {
  const together = finishedAfter(
    NOTHING_FINISHED,
    answer({ freeze: freeze(1, 'done'), sync: sync(1, 'done'), delete: deletion(1, 'done') }),
  );
  expect(together.order).toEqual(['freeze', 'sync', 'delete']);
});

// Run numbers start again at 1 with a new process, so a name that changed
// forgets what was seen rather than measuring the new runs against it.
it('forgets what it saw when another process answers', () => {
  const before = finishedAfter(NOTHING_FINISHED, answer({ sync: sync(4, 'done') }));
  const after = finishedAfter(before, answer({ server: 'b2', freeze: freeze(1, 'done') }));
  expect(after.order).toEqual(['freeze']);
  expect(after.seen.sync).toBe(0);
});

it('ranks every flow, the latest first and the rest in the old order', () => {
  expect(latestFirst(['delete'])).toEqual(['delete', 'freeze', 'sync']);
  expect(latestFirst([])).toEqual(['freeze', 'sync', 'delete']);
});
