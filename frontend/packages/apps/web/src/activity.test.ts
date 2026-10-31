import { expect, it } from 'vitest';

import type { Delete, Fill, Freeze, Logged, LoggedWork, Refused, Sync } from '@coffret/api';

import {
  ANSWER_HOLDS,
  answering,
  collapsed,
  earlier,
  entryOf,
  failedBadge,
  firstStanding,
  inProgress,
  offeredNow,
  retryOf,
  stillStanding,
  stoppedLine,
  type Entry,
  type InFlight,
  type PageNotice,
  type Running,
} from './activity';

/** What stops a run in these cases unless a case says otherwise. */
const STORAGE: Refused = {
  kind: 'storage',
  message: "the Library's Storage did not answer",
  reason: null,
  surfaced: null,
};

/** A conflict: another device's commit got in front of a deletion. */
const CONFLICT: Refused = {
  kind: 'conflict',
  message: 'another device changed the Library meanwhile',
  reason: null,
  surfaced: null,
};

function aFill(stopped: Refused | null, over: Partial<Fill> = {}): Fill {
  const own = {
    run: 1,
    folder: 'albums',
    total: 4,
    done: 4,
    declined: [],
    findings: [],
    waiting: [],
    discarded: [],
    displaced: [],
    stop: null,
  };
  const standing =
    stopped === null
      ? { status: 'done' as const, stopped: null }
      : { status: 'stopped' as const, stopped };
  return { ...own, ...over, ...standing } as Fill;
}

function aSync(stopped: Refused | null): Sync {
  const own = { run: 1, added: 2, findings: [], step: null, waits: false, stop: null };
  return stopped === null
    ? { ...own, status: 'done', stopped: null }
    : { ...own, status: 'stopped', stopped };
}

function aFreeze(stopped: Refused | null, status: 'done' | 'freezing' = 'done'): Freeze {
  const own = {
    run: 1,
    folder: 'books/vol-1',
    packs: 1,
    entries: 188,
    findings: [],
    step: null,
    waits: false,
    waiting: [],
    stopping: [],
    discarded: [],
    displaced: [],
    stop: null,
  };
  return stopped === null
    ? { ...own, status, stopped: null }
    : { ...own, status: 'stopped', stopped };
}

function aDeletion(stopped: Refused | null): Delete {
  const own = {
    run: 1,
    folders: ['albums'],
    paths: [],
    entries: 5,
    bytes: 8_200_000,
    removed: 5,
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
  return stopped === null
    ? { ...own, status: 'done', stopped: null }
    : { ...own, status: 'stopped', stopped };
}

/** One entry of the log, logged at minute `minute` past nine. */
function logged(work: LoggedWork, id = 1, settled = false, minute = id): Logged {
  const at = `2026-10-30T09:${String(minute).padStart(2, '0')}:00Z`;
  return { id, at, settled, ...work };
}

function running(text: string, progress: number | null = null): Running {
  return { key: text, text, progress, stop: null, ending: null };
}

/** An entry as the panel draws it, failed and standing where `standing` says. */
function anEntry(text: string, standing = false): Entry {
  return {
    key: text,
    at: '2026-10-30T09:00:00Z',
    outcome: standing ? 'failure' : 'success',
    text,
    retry: null,
    retryAs: null,
    standing,
    settled: false,
    ranOut: false,
    answers: false,
  };
}

const NOTHING: InFlight = {
  adding: null,
  addingAs: null,
  adds: [],
  sent: null,
  turn: null,
  fill: null,
  sync: null,
  freeze: null,
  deletion: null,
  fetching: null,
};

// With nothing in progress the row says the newest thing that
// ended, the answer to whatever just happened; where nothing has ended it says
// nothing at all, since being idle is the usual state.
it('summarizes nothing running by the newest entry, or says nothing', () => {
  expect(collapsed([], [])).toEqual({ text: '', progress: null, failed: 0, said: null });
  const newest = anEntry('added 3 files to the Library');
  expect(collapsed([], [newest, anEntry('older')])).toEqual({
    text: 'added 3 files to the Library',
    progress: null,
    failed: 0,
    said: newest,
  });
});

// One thing in progress is said whole, with how far it has got.
it('summarizes one run by its own line and progress', () => {
  expect(collapsed([running('packing books/vol-1 — packing 3/12…', 0.25)], [])).toEqual({
    text: 'packing books/vol-1 — packing 3/12…',
    progress: 0.25,
    failed: 0,
    said: null,
  });
});

// Several in progress: the first speaks for the rest, with a count of them.
it('summarizes several runs by the first and a count of the rest', () => {
  const row = collapsed(
    [running('sending 188 files to Vol. 06 — 112 of 188', 0.6), running('backing up what was added…')],
    [],
  );
  expect(row.text).toBe('sending 188 files to Vol. 06 — 112 of 188 · 1 more');
  expect(row.progress).toBe(0.6);

  expect(collapsed([running('a'), running('b'), running('c')], []).text).toBe('a · 2 more');
});

// Failures are counted for the badge whatever is running, and only the ones
// standing: a failure a later run settled is history.
it('counts the failures standing for the badge', () => {
  const entries = [anEntry('could not pack', true), anEntry('done'), anEntry('could not fetch', true)];
  expect(collapsed([running('backing up what was added…')], entries).failed).toBe(2);
  expect(collapsed([], entries).failed).toBe(2);
  expect(failedBadge(2)).toBe('2 failed');
  expect(failedBadge(0)).toBeNull();

  const settled = entryOf(logged({ kind: 'fill', run: aFill(STORAGE) }, 1, true), false);
  expect(settled.standing).toBe(false);
  expect(collapsed([], [settled]).failed).toBe(0);
});

// What offers a Retry: a stopped run asking again could help, and a folder a
// queue lost — each until a later run of the same work settles it, and none
// while the Library is locked.
it('offers a Retry on the failures asking again could help', () => {
  expect(retryOf(logged({ kind: 'fill', run: aFill(STORAGE) }), false)).toEqual({
    flow: 'fill',
    folder: 'albums',
  });
  expect(retryOf(logged({ kind: 'sync', run: aSync(STORAGE) }), false)).toEqual({ flow: 'sync' });
  expect(retryOf(logged({ kind: 'freeze', run: aFreeze(STORAGE) }), false)).toEqual({
    flow: 'freeze',
    folder: 'books/vol-1',
  });
  expect(retryOf(logged({ kind: 'discarded', queue: 'fill', folder: 'letters' }), false)).toEqual({
    flow: 'fill',
    folder: 'letters',
  });
  expect(retryOf(logged({ kind: 'discarded', queue: 'freeze', folder: 'books/vol-2' }), false)).toEqual(
    { flow: 'freeze', folder: 'books/vol-2' },
  );
  // A deletion only where another device's commit got in front of it.
  expect(retryOf(logged({ kind: 'delete', run: aDeletion(CONFLICT) }), false)).toEqual({
    flow: 'delete',
    target: { folders: ['albums'], paths: [] },
  });
});

it('offers no Retry where asking again could not help, or nothing failed', () => {
  // Work that went as asked.
  expect(retryOf(logged({ kind: 'fill', run: aFill(null) }), false)).toBeNull();
  expect(retryOf(logged({ kind: 'sync', run: aSync(null) }), false)).toBeNull();
  expect(retryOf(logged({ kind: 'freeze', run: aFreeze(null) }), false)).toBeNull();
  expect(retryOf(logged({ kind: 'delete', run: aDeletion(null) }), false)).toBeNull();
  // A refused root, an epoch, a lock: met again by asking again.
  const root: Refused = { ...STORAGE, kind: 'refused_placement', reason: 'refused_root' };
  expect(retryOf(logged({ kind: 'fill', run: aFill(root) }), false)).toBeNull();
  expect(retryOf(logged({ kind: 'sync', run: aSync({ ...STORAGE, kind: 'epoch' }) }), false)).toBeNull();
  // A deletion Storage stopped: its sentence does not say to run it again.
  expect(retryOf(logged({ kind: 'delete', run: aDeletion(STORAGE) }), false)).toBeNull();
  // A refused fetch is the reader's, asked again by opening the page.
  expect(
    retryOf(logged({ kind: 'fetch', path: 'albums/cover.png', refused: STORAGE }), false),
  ).toBeNull();
  // Settled by a later run of the same work.
  expect(retryOf(logged({ kind: 'fill', run: aFill(STORAGE) }, 1, true), false)).toBeNull();
  // And nothing at all while the Library is locked.
  expect(retryOf(logged({ kind: 'fill', run: aFill(STORAGE) }), true)).toBeNull();
  expect(retryOf(logged({ kind: 'discarded', queue: 'fill', folder: 'letters' }), true)).toBeNull();
});

// Each Retry is named by what it would do again, so two side by side are told
// apart — and a folder a queue lost is named, being no run's folder.
it('names each Retry by what it would do again', () => {
  const words = (work: LoggedWork) => entryOf(logged(work), false).retryAs;
  expect(words({ kind: 'fill', run: aFill(STORAGE) })).toBe('bring over again');
  expect(words({ kind: 'sync', run: aSync(STORAGE) })).toBe('back up again');
  expect(words({ kind: 'freeze', run: aFreeze(STORAGE) })).toBe('pack again');
  expect(words({ kind: 'discarded', queue: 'fill', folder: 'letters' })).toBe('bring over letters');
  expect(words({ kind: 'discarded', queue: 'freeze', folder: '' })).toBe('pack the Library root');
  expect(words({ kind: 'delete', run: aDeletion(CONFLICT) })).toBe('try again');
  expect(words({ kind: 'fill', run: aFill(null) })).toBeNull();
});

// A Retry goes as the work it asks for is taken up — running, or waiting its
// turn — while the failure it stands beside goes on standing until that work
// settles it; a Retry for other work stays.
it('takes a Retry away while its work is under way or queued', () => {
  const entries = [
    entryOf(logged({ kind: 'fill', run: aFill(STORAGE) }, 1), false),
    entryOf(logged({ kind: 'sync', run: aSync(STORAGE) }, 2), false),
    entryOf(logged({ kind: 'freeze', run: aFreeze(STORAGE) }, 3), false),
    entryOf(logged({ kind: 'discarded', queue: 'fill', folder: 'letters' }, 4), false),
  ];
  const offered = (flight: Pick<InFlight, 'fill' | 'sync' | 'freeze'>) =>
    offeredNow(entries, flight).map((entry) => entry.retryAs);

  expect(offered({ fill: null, sync: null, freeze: null })).toEqual([
    'bring over again',
    'back up again',
    'pack again',
    'bring over letters',
  ]);

  const filling = { ...aFill(null), status: 'filling', stopped: null, waiting: ['letters'] } as Fill;
  const syncing = { ...aSync(null), status: 'syncing', stopped: null } as Sync;
  expect(
    offered({ fill: filling, sync: syncing, freeze: aFreeze(null, 'freezing') }),
  ).toEqual([null, null, null, null]);
  expect(offeredNow(entries, { fill: filling, sync: syncing, freeze: null })[0]?.standing).toBe(true);

  const elsewhere = { ...filling, folder: 'photos', waiting: [] } as Fill;
  expect(offered({ fill: elsewhere, sync: null, freeze: null })).toEqual([
    'bring over again',
    'back up again',
    'pack again',
    'bring over letters',
  ]);
});

// Earlier is the server's log and this page's notices together, newest first,
// each with its outcome.
it('lists the log and the page’s notices together, newest first', () => {
  const notice: PageNotice = {
    id: 1,
    at: '2026-10-30T09:05:00Z',
    text: 'cover.pdf — preview of this format is not supported yet',
    outcome: 'notice',
    retry: null,
    settled: false,
  };
  const entries = earlier(
    [
      logged({ kind: 'freeze', run: aFreeze(STORAGE) }, 3, false, 9),
      logged({ kind: 'sync', run: aSync(null) }, 2, false, 2),
    ],
    [notice],
    false,
  );
  expect(entries.map((entry) => [entry.text, entry.outcome])).toEqual([
    [`could not pack books/vol-1 — ${STORAGE.message}`, 'failure'],
    ['cover.pdf — preview of this format is not supported yet', 'notice'],
    ['added 2 files to the Library', 'success'],
  ]);
});

// LA-33: what a read that armed no fill found is said among the rest, as a
// notice with nothing to retry: the read was answered.
it('lists what reads that armed no fill found among the rest, as notices', () => {
  const entries = earlier(
    [logged({ kind: 'sync', run: aSync(null) }, 2, false, 2)],
    [],
    false,
    [
      {
        id: 1,
        at: '2026-10-30T09:05:00Z',
        path: null,
        message: 'the Library’s Keyring is degraded',
        reason: 'keyring_degraded',
        surfaced: null,
      },
    ],
  );
  expect(entries.map((entry) => [entry.key, entry.text, entry.outcome, entry.retry, entry.standing])).toEqual([
    ['found:1', 'the Library’s Keyring is degraded', 'notice', null, false],
    ['server:2', 'added 2 files to the Library', 'success', null, false],
  ]);
});

// A fill that brought everything over has nothing on its running line to say,
// but an entry of what happened says what it was.
it('says what a quiet fill brought over', () => {
  expect(entryOf(logged({ kind: 'fill', run: aFill(null) }), false).text).toBe(
    'brought over 4 files in albums',
  );
});

// A deletion's entry is the sentence the explorer always said of one.
it('says what a deletion removed', () => {
  const entry = entryOf(logged({ kind: 'delete', run: aDeletion(null) }), false);
  expect(entry.text).toContain('removed from the Library');
  expect(entry.outcome).toBe('success');
});

// LA-12: a catch-up the explorer asked for is in the list like the rest of the
// log — a refused one a failure standing until the next press settles it, with
// nothing offered again here, and one that landed said as its answer was.
it('lists a refused Check for changes as a failure, and a landed one as its answer', () => {
  const refused = entryOf(logged({ kind: 'catch_up', refreshed: null, refused: STORAGE }), false);
  expect(refused.text).toBe("could not check for changes — the Library's Storage did not answer");
  expect(refused.outcome).toBe('failure');
  expect(refused.standing).toBe(true);
  expect(refused.retry).toBeNull();

  const landed = entryOf(
    logged({ kind: 'catch_up', refreshed: { advanced: true, gained: 3, entries: 120 }, refused: null }),
    false,
  );
  expect(landed.text).toBe('checked for changes — 3 new files');
  expect(landed.outcome).toBe('success');
  expect(landed.standing).toBe(false);

  const settled = entryOf(logged({ kind: 'catch_up', refreshed: null, refused: STORAGE }, 1, true), false);
  expect(settled.standing).toBe(false);
});

// In progress, in the order it is happening: the drop, the run
// holding the turn, the ones waiting for it, the fill, and the reader's own
// fetch with its own Stop beside them.
it('lists what is in progress in the order it is happening', () => {
  const freezing = aFreeze(null, 'freezing');
  const syncing: Sync = { ...aSync(null), status: 'syncing', stopped: null, waits: true };
  const filling: Fill = { ...aFill(null, { done: 1, total: 4 }), status: 'filling', stopped: null };
  const listed = inProgress({
    ...NOTHING,
    adding: 'sending 3 files to albums',
    sent: 0.5,
    turn: 'freeze',
    freeze: freezing,
    sync: syncing,
    fill: filling,
    fetching: 'cover.png',
  });
  expect(listed.map((item) => item.key)).toEqual(['adding', 'freeze', 'sync', 'fill', 'fetching']);
  expect(listed[4].stop).toEqual({ reader: true });
  expect(listed[0].progress).toBe(0.5);
  expect(listed[2].text).toContain('waiting for the packing under way to finish');
  expect(listed[3].progress).toBe(0.25);

  expect(inProgress({ ...NOTHING, fetching: 'cover.png' })).toEqual([
    {
      key: 'fetching',
      text: 'fetching cover.png…',
      progress: null,
      stop: { reader: true },
      ending: null,
    },
  ]);
  expect(inProgress(NOTHING)).toEqual([]);
});

// A refused press stands while an entry still offers what it pressed.
it('keeps a refused press while its entry still offers it', () => {
  const trouble = { pressed: { flow: 'freeze' as const, folder: 'books/vol-1' }, said: 'locked' };
  const stopped = logged({ kind: 'freeze', run: aFreeze(STORAGE) });
  expect(stillStanding(trouble, [stopped])).toBe(trouble);
  expect(stillStanding(trouble, [{ ...stopped, settled: true }])).toBeNull();
  expect(stillStanding(trouble, [])).toBeNull();
  expect(stillStanding(null, [stopped])).toBeNull();
});

/** This page's answer to a gesture, stamped at `at`. */
function anAnswer(text: string, at: string, outcome: Entry['outcome'] = 'notice'): Entry {
  return { ...anEntry(text), key: `page:${text}`, at, outcome, answers: true };
}

// The answer to a gesture takes the collapsed row ahead of what is running —
// keeping the bar and the badge — until the next gesture or its moment ends,
// and the row then says what it said before.
it('lets the answer to a gesture take the row until the next gesture or a few seconds', () => {
  const at = '2026-10-30T09:00:00.500Z';
  const stamped = Date.parse(at);
  const gesture = stamped - 20;
  const unsupported = anAnswer('preview of this format is not supported yet', at);
  const failure = anEntry('could not pack', true);
  const entries = [unsupported, failure];
  const work = [running('backing up what was added…', 0.4)];

  const shown = answering(entries, stamped + 10, gesture);
  expect(shown).toEqual({ entry: unsupported, until: stamped + ANSWER_HOLDS });
  expect(collapsed(work, entries, shown?.entry ?? null)).toEqual({
    text: 'preview of this format is not supported yet',
    progress: 0.4,
    failed: 1,
    said: unsupported,
  });

  expect(answering(entries, stamped + ANSWER_HOLDS - 1, gesture)?.entry).toBe(unsupported);
  expect(answering(entries, stamped + ANSWER_HOLDS, gesture), 'its moment ran out').toBeNull();
  expect(answering(entries, stamped + 1000, stamped + 900), 'a later gesture took it').toBeNull();
  expect(collapsed(work, entries, null).text).toBe('backing up what was added…');

  // Stamped in the same millisecond as the gesture it answers, it is that gesture's.
  expect(answering(entries, stamped, stamped)?.entry).toBe(unsupported);
});

// Only this page's own answers take the row, and only the newest of them,
// unsettled: the server's word on its work is not an answer to a gesture.
it('lets only the newest unsettled answer of this page take the row', () => {
  const at = '2026-10-30T09:00:00.000Z';
  const now = Date.parse(at) + 10;
  const server = { ...anEntry('could not pack', true), at };
  expect(answering([server], now, 0)).toBeNull();

  const older = anAnswer('older', '2026-10-30T08:59:59.000Z');
  const newer = anAnswer('could not delete', at, 'failure');
  expect(answering([server, newer, older], now, 0)?.entry).toBe(newer);
  expect(answering([{ ...newer, settled: true }, older], now, 0)).toBeNull();
});

// The badge takes a person to the newest failure standing, past settled ones.
it('finds the first failure standing for the badge', () => {
  const settled = { ...anEntry('could not fetch', true), standing: false, settled: true };
  const standing = anEntry('could not pack', true);
  expect(firstStanding([anEntry('done'), settled, standing, anEntry('older', true)])).toBe(standing);
  expect(firstStanding([anEntry('done')])).toBeNull();
});

// LA-34, LA-35: the lines a person can stop carry a Stop naming what it
// stops — the drop being sent by its name, a run by its number — and a
// deletion carries none.
it('offers a Stop on an add, a freeze and a fill, and none on a deletion', () => {
  const listed = inProgress({
    ...NOTHING,
    adding: 'sending 3 files to albums',
    addingAs: { name: 'drop-1', stopping: false },
    turn: 'sync',
    sync: { ...aSync(null), run: 4, status: 'syncing', stopped: null, stop: 'offered' },
    freeze: { ...aFreeze(null, 'freezing'), run: 2, stop: 'offered' },
    fill: { ...aFill(null, { done: 1 }), run: 7, status: 'filling', stopped: null, stop: 'offered' },
    deletion: { ...aDeletion(null), status: 'deleting', stopped: null },
  });
  const stops = Object.fromEntries(listed.map((item) => [item.key, item.stop]));
  expect(stops).toEqual({
    adding: { server: { add: 'drop-1' } },
    sync: { server: { sync: 4 } },
    freeze: { server: { freeze: 2 } },
    fill: { server: { fill: 7 } },
    delete: null,
  });
  expect(listed.every((item) => item.ending === null)).toBe(true);
});

// LA-34: a book waiting in the freeze's queue and a folder waiting in the
// fill's each draw a line of their own under their run, with the Stop that
// names them; a book holding adds a person asked to stop says it is stopping.
it('offers a Stop on a book or a folder waiting in its queue', () => {
  const listed = inProgress({
    ...NOTHING,
    freeze: {
      ...aFreeze(null, 'freezing'),
      run: 2,
      stop: 'offered',
      waiting: ['books/vol-2', 'books/vol-3'],
      stopping: ['books/vol-3'],
    },
    fill: {
      ...aFill(null, { done: 1, waiting: ['letters'] }),
      run: 7,
      status: 'filling',
      stopped: null,
      stop: 'offered',
    },
  });
  expect(listed.map((item) => [item.key, item.text, item.stop, item.ending])).toEqual([
    ['freeze', 'packing books/vol-1…', { server: { freeze: 2 } }, null],
    ['queued-freeze:books/vol-2', 'waiting to pack books/vol-2', { server: { queued_freeze: 'books/vol-2' } }, null],
    ['queued-freeze:books/vol-3', 'waiting to pack books/vol-3', null, 'stopping'],
    ['fill', 'bringing over 1/4 in albums…', { server: { fill: 7 } }, null],
    ['queued-fill:letters', 'waiting to bring over letters', { server: { queued_fill: 'letters' } }, null],
  ]);
});

// LA-35, LA-12: a folder stopped while it waited in the fill's queue is
// logged cancelled, having brought nothing over, and offers no Retry.
it('says a fill stopped in its queue brought nothing over', () => {
  const run = { ...aFill(null, { run: 0, total: 0, done: 0 }), status: 'cancelled', stopped: null } as Fill;
  const [entry] = earlier([logged({ kind: 'fill', run })], [], false);
  expect([entry.text, entry.outcome, entry.retry]).toEqual([
    'stopped bringing over albums — nothing was brought over',
    'notice',
    null,
  ]);
});

// LA-35: once the run has begun committing, its line says it is finishing
// and offers no Stop; a stop asked then is answered by the run's real end,
// which is the sync's own entry and no stopped add.
it('says a run is finishing once it commits, and is answered by its real end', () => {
  const committing = inProgress({
    ...NOTHING,
    sync: { ...aSync(null), status: 'syncing', stopped: null, stop: 'committing' },
  });
  expect(committing).toMatchObject([{ key: 'sync', stop: null, ending: 'finishing' }]);

  const ended = earlier([logged({ kind: 'sync', run: { ...aSync(null), added: 1 } })], [], false);
  expect(ended.map((entry) => entry.text)).toEqual(['added 1 file to the Library']);
});

// LA-35, LA-36: a stop whose answer this page never read — it was reloaded
// mid-stop — is still shown being answered, from the server's own account:
// the run asked to stop, or the add still being received, says it is
// stopping; and once it has ended the log says what the stop came to, with no
// Retry, since adding again is dropping again.
it('shows a stop being answered after a reload, and then what it came to', () => {
  const asked = inProgress({
    ...NOTHING,
    sync: { ...aSync(null), status: 'syncing', stopped: null, stop: 'asked' },
  });
  expect(asked).toMatchObject([{ key: 'sync', stop: null, ending: 'stopping' }]);

  const receiving = inProgress({
    ...NOTHING,
    adds: [{ id: 'drop-1', folder: 'albums', stage: 'receiving', stopping: true }],
  });
  expect(receiving).toMatchObject([{ text: 'adding to albums', stop: null, ending: 'stopping' }]);

  const after = inProgress({ ...NOTHING, sync: { ...aSync(null), status: 'cancelled', stopped: null } });
  expect(after).toEqual([]);
  const stopped = logged({
    kind: 'add',
    add: { folder: 'albums', files: 3, removed: 3, committed: 0, left: 0 },
  });
  const [entry] = earlier([stopped], [], false);
  expect(entry.text).toBe('stopped adding to albums — nothing was added to the Library');
  expect(entry.outcome).toBe('notice');
  expect(entry.retry).toBeNull();
  expect(entry.standing).toBe(false);
});

// The page's own Stop on the drop being sent says it is stopping at once,
// and so does one the server says another page asked.
it('says the drop being sent is stopping once its Stop is pressed', () => {
  const pressed = inProgress({
    ...NOTHING,
    adding: 'sending 3 files to albums',
    addingAs: { name: 'drop-1', stopping: true },
  });
  expect(pressed).toMatchObject([{ key: 'adding', stop: null, ending: 'stopping' }]);
  const elsewhere = inProgress({
    ...NOTHING,
    adding: 'sending 3 files to albums',
    addingAs: { name: 'drop-1', stopping: false },
    adds: [{ id: 'drop-1', folder: 'albums', stage: 'receiving', stopping: true }],
  });
  expect(elsewhere).toMatchObject([{ key: 'adding', stop: null, ending: 'stopping' }]);
});

// LA-36: what a stopped add says — nothing more where every file it wrote was
// taken away, a file it wrote over put back among them; how much a run beside
// it had carried in already; and how many this device could not take back out
// of the folder, for a sync whose scope covers them to carry in (spec: EP-10).
it('says what a stopped add left behind', () => {
  expect(stoppedLine({ folder: 'albums', files: 3, removed: 3, committed: 0, left: 0 })).toBe(
    'stopped adding to albums — nothing was added to the Library',
  );
  expect(stoppedLine({ folder: 'albums', files: 4, removed: 2, committed: 1, left: 1 })).toBe(
    'stopped adding to albums — 1 file had been added to the Library already; 1 file could not be taken back out of the folder, and a sync that covers it adds it — a full sync, or a later drop into that folder',
  );
  expect(stoppedLine({ folder: 'albums', files: 2, removed: 0, committed: 0, left: 2 })).toBe(
    'stopped adding to albums — nothing was added to the Library; 2 files could not be taken back out of the folder, and a sync that covers them adds them — a full sync, or a later drop into that folder',
  );
  expect(stoppedLine({ folder: '', files: 1, removed: 1, committed: 0, left: 0 })).toBe(
    'stopped adding to the Library root — nothing was added to the Library',
  );
});

// LA-35, LA-37: a fill and a freeze a person stopped say so, and offer no Retry.
it('says a stopped fill and freeze stopped, and offers nothing again', () => {
  const fill = logged({ kind: 'fill', run: { ...aFill(null), status: 'cancelled', stopped: null } }, 1);
  const freeze = logged(
    { kind: 'freeze', run: { ...aFreeze(null), status: 'cancelled', stopped: null } },
    2,
  );
  const entries = earlier([freeze, fill], [], false);
  expect(entries.map((entry) => [entry.text, entry.outcome, entry.retry])).toEqual([
    ['stopped packing books/vol-1 — nothing was packed', 'notice', null],
    ['stopped bringing over albums — what was brought over stays', 'notice', null],
  ]);
});
