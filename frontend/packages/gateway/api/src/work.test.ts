import { afterEach, expect, it, vi } from 'vitest';

import findingReasons from './finding-reasons.json';
import type { Finding, FindingReason } from './work';
import { previewDelete, startDelete, workOf } from './work';
import type { PlacementReason, SurfacedFinding } from './refusal';
import surfacedFindings from './surfaced-findings.json';

// The shape a page reads a finding in, as the server's route test pins it on
// the wire: the Entry, the sentence, and which finding it is in the two fields
// a declined fetch names one by. Read through the decoder a page's answers go
// through, so a field renamed or dropped on one side fails here, and the keys
// are asserted so that a field added on this side alone fails too.
it('reads a finding as its Entry, its sentence, its reason and what it surfaced', () => {
  const { sync } = workOf(
    answer({
      sync: {
        run: 2,
        status: 'done',
        added: 0,
        findings: [
          {
            path: 'albums/gone.jpg',
            message: 'this device had this file and it is gone; the Library still holds it',
            reason: 'surfaced',
            surfaced: 'DeletedLocally',
          },
          {
            path: null,
            message: 'a folder this device maps is not there, so nothing in it was looked at',
            reason: 'root_missing',
          },
        ],
        step: null,
        stopped: null,
      },
    }),
  );
  const [file, root] = sync?.findings ?? [];

  expect(Object.keys(file).sort()).toEqual(['message', 'path', 'reason', 'surfaced']);
  expect(file.surfaced).toBe('DeletedLocally');
  expect(Object.keys(root).sort()).toEqual(['message', 'path', 'reason', 'surfaced']);
  expect(root.surfaced).toBeNull();
});

// A fill — the one run a person who only opens files makes — carries findings
// in the sync's shape and reads them through the same decoder: what its reads
// found of the Library's Keyring. A displaced fill carries them too, and a
// reason this build does not know reads as `null`, as a sync's does.
it("reads a fill's findings, and a displaced fill's, as a sync's are read", () => {
  const degraded = {
    path: null,
    message: "the Library's Keyring is degraded",
    reason: 'keyring_degraded',
  };
  const { fill } = workOf(
    answer({
      fill: {
        run: 3,
        folder: 'albums',
        status: 'done',
        total: 2,
        done: 2,
        declined: [],
        findings: [degraded],
        waiting: [],
        discarded: [],
        displaced: [
          {
            run: 2,
            folder: 'letters',
            status: 'stopped',
            total: 2,
            done: 0,
            declined: [],
            findings: [{ ...degraded, reason: 'something_new' }],
            stopped: { error: 'storage', message: 'Storage did not answer' },
          },
        ],
        stopped: null,
      },
    }),
  );

  expect(fill?.findings).toEqual([{ ...degraded, surfaced: null }]);
  expect(fill?.displaced[0].findings).toEqual([{ ...degraded, reason: null, surfaced: null }]);
});

// A sync whose commit failed after it repaired the Keyring carries the repair
// on the run that stopped, and the decoder keeps it beside the refusal rather
// than dropping a stopped run's findings (spec: KL-15).
it("reads a stopped sync's Keyring repair beside what stopped it", () => {
  const repaired = {
    path: null,
    message:
      'repaired the Keyring: 1 replica of generation 4 was missing or unreadable, ' +
      'and was rewritten from a surviving one',
    reason: 'keyring_repaired',
  };
  const { sync } = workOf(
    answer({
      sync: {
        run: 3,
        status: 'stopped',
        added: 0,
        findings: [repaired],
        step: null,
        stopped: { error: 'storage', message: 'Storage did not answer' },
      },
    }),
  );

  expect(sync?.status).toBe('stopped');
  expect(sync?.findings).toEqual([{ ...repaired, surfaced: null }]);
});

// The reasons as the shared file holds them, against the union a caller
// branches on. The server holds the file to what it can build; this holds the
// union to the file, one literal per reason, so a reason the server grew is a
// line here that fails until the union has it too.
it('names every reason the server can send', () => {
  const reasons: FindingReason[] = [
    'surfaced',
    'key_lost',
    'root_missing',
    'root_on_another_filesystem',
    'refused_root',
    'keyring_degraded',
    'keyring_repaired',
  ];

  expect(reasons).toEqual(findingReasons);
});

// One state, one spelling: every finding reason a refusal can also carry is
// the refusal's own literal. The ones a finding shares are typed as
// `PlacementReason`, so a refusal spelling that moved fails to compile here, and
// the rest of the file is exactly the run's own four.
it('spells every reason a refusal also carries as the refusal does', () => {
  const shared: PlacementReason[] = ['surfaced', 'key_lost', 'refused_root'];
  const runOnly: FindingReason[] = [
    'root_missing',
    'root_on_another_filesystem',
    'keyring_degraded',
    'keyring_repaired',
  ];

  expect(
    findingReasons.filter((reason) => !(shared as string[]).includes(reason)),
  ).toEqual(runOnly);
});

// A lost key is one state on both routes: `key_lost` beside `KeyLost`, as the
// refusal a fetch declines it with pairs them. And the two names only a sync
// finds are in the refusals' file beside it rather than in a list of their own.
it('pairs a lost key the way a refusal does, and keeps the sync-only names in the refusals file', () => {
  const lost: Finding = {
    path: 'albums/a.jpg',
    message: 'the Library records no key for the Container holding this file',
    reason: 'key_lost',
    surfaced: 'KeyLost',
  };
  const names = surfacedFindings as SurfacedFinding[];

  expect(names).toContain(lost.surfaced);
  expect(names).toContain('ChangedInPack');
  expect(names).toContain('DeletedLocally');
});

/** A work answer as the server sends it, with nothing running but `runs`. */
function answer(runs: Record<string, unknown>): unknown {
  return {
    server: 'a-server',
    library: 'unlocked',
    catalog: { state: 'caught_up', stopped: null },
    fill: null,
    sync: null,
    freeze: null,
    delete: null,
    reconnect: null,
    ...runs,
  };
}

// A refusal inside an answer lands where a refused request's would: a kind this
// client has not heard of is `unrecognized`, and a reason or a finding name it
// has not heard of is `null`. Every refusal the answer carries goes through it —
// what stopped a run, what stopped a displaced one, an Entry a fill declined and
// what stopped the catalog — so a screen comparing one of them against a union
// is never handed a string outside it.
it('narrows a refusal the work answer carries as a refused request is narrowed', () => {
  const grown = { error: 'quota', message: 'a kind this page has never heard of' };
  const reasoned = {
    error: 'declined',
    message: 'a reason this page has never heard of',
    reason: 'elsewhere',
    surfaced: 'SomethingNew',
  };
  const read = workOf(
    answer({
      catalog: { state: 'behind', stopped: grown },
      fill: {
        run: 2,
        folder: 'albums',
        status: 'stopped',
        total: 3,
        done: 1,
        declined: [{ path: 'albums/a.jpg', ...reasoned }],
        waiting: [],
        discarded: [],
        displaced: [
          {
            run: 1,
            folder: 'letters',
            status: 'stopped',
            total: 2,
            done: 0,
            declined: [],
            stopped: grown,
          },
        ],
        stopped: grown,
      },
      freeze: {
        run: 1,
        folder: 'books',
        status: 'done',
        packs: 0,
        entries: 0,
        findings: [
          { path: 'books/a.jpg', message: 'found', reason: 'misplaced', surfaced: 'Elsewhere' },
        ],
        step: null,
        waiting: [],
        discarded: [],
        displaced: [],
        stopped: null,
      },
    }),
  );

  const unrecognized = {
    kind: 'unrecognized',
    message: grown.message,
    reason: null,
    surfaced: null,
  };
  expect(read.catalog.stopped).toEqual(unrecognized);
  expect(read.fill?.stopped).toEqual(unrecognized);
  expect(read.fill?.displaced[0].stopped).toEqual(unrecognized);
  expect(read.fill?.declined).toEqual([
    {
      path: 'albums/a.jpg',
      kind: 'declined',
      message: reasoned.message,
      reason: null,
      surfaced: null,
    },
  ]);
  expect(read.freeze?.findings).toEqual([
    { path: 'books/a.jpg', message: 'found', reason: null, surfaced: null },
  ]);
});

// How the last reconnect ended rides beside the runs, and an answer from a
// server that has never reconnected reads as none.
it('reads the reconnect the work answer carries, and none where it carries none', () => {
  const waiting = workOf(
    answer({ reconnect: { state: 'waiting', message: 'waiting for the consent page' } }),
  );
  expect(waiting.reconnect).toEqual({ state: 'waiting', message: 'waiting for the consent page' });

  expect(workOf(answer({ reconnect: null })).reconnect).toBeNull();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

// A deletion is read like every other run: what stopped it narrowed as a
// refusal is, and a refused Pack's reason this client has not heard of is
// `null` rather than a string claiming the union.
it('reads a deletion, its refused Packs and what stopped it', () => {
  const conflict = {
    error: 'conflict',
    message: 'another device changed the Library meanwhile, so nothing was changed',
  };
  const read = workOf(
    answer({
      delete: {
        run: 3,
        folder: null,
        paths: ['books/a.jpg'],
        status: 'stopped',
        entries: 0,
        bytes: 0,
        removed: 0,
        rebuilt: 0,
        rebuild_read: 0,
        rebuild_written: 0,
        refused: [
          { spared: ['books/a.jpg'], kept: 2, reason: 'key_lost', message: 'no key' },
          { spared: ['books/b.jpg'], kept: 1, reason: 'eaten', message: 'something new' },
        ],
        missing: [],
        findings: [],
        step: null,
        waiting: 0,
        stopped: conflict,
      },
    }),
  );

  expect(read.delete?.status).toBe('stopped');
  expect(read.delete?.stopped?.kind).toBe('conflict');
  expect(read.delete?.refused.map((refused) => refused.reason)).toEqual(['key_lost', null]);
});

// What a deletion names goes on the query: the folder as `path`, each file as
// an `entry` of its own — the same for the preview and for the run, so the two
// cannot name different things.
it('names a deletion by its folder and its files, the same way both times', async () => {
  const asked: { url: string; method: string }[] = [];
  vi.stubGlobal('fetch', (url: string, init?: RequestInit) => {
    asked.push({ url, method: init?.method ?? 'GET' });
    const body = url.includes('delete')
      ? init?.method === 'POST'
        ? answer({})
        : {
            folder: 'albums',
            paths: ['books/a b.jpg'],
            entries: 1,
            bytes: 5,
            removed: 1,
            rebuilt: 0,
            rebuild_read: 0,
            rebuild_written: 0,
            refused: [],
            missing: [],
            after_current: false,
          }
      : {};
    return Promise.resolve(new Response(JSON.stringify(body), { status: 200 }));
  });

  const target = { folder: 'albums', paths: ['books/a b.jpg'] };
  await previewDelete(target);
  await startDelete(target);

  expect(asked.map((request) => request.method)).toEqual(['GET', 'POST']);
  for (const { url } of asked) {
    const query = new URL(url, 'http://127.0.0.1').searchParams;
    expect(query.getAll('path')).toEqual(['albums']);
    expect(query.getAll('entry')).toEqual(['books/a b.jpg']);
  }
});
