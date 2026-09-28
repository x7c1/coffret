import { expect, it } from 'vitest';

import findingReasons from './finding-reasons.json';
import type { Finding, FindingReason } from './work';
import { workOf } from './work';
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

// The reasons as the shared file holds them, against the union a caller
// branches on. The server holds the file to what it can build; this holds the
// union to the file, one literal per reason, so a reason the server grew is a
// line here that fails until the union has it too.
it('names every reason the server can send', () => {
  const reasons: FindingReason[] = [
    'surfaced',
    'locked',
    'root_missing',
    'root_on_another_filesystem',
    'refused_root',
    'keyring_degraded',
  ];

  expect(reasons).toEqual(findingReasons);
});

// One state, one spelling: every finding reason a refusal can also carry is
// the refusal's own literal. The ones a finding shares are typed as
// `PlacementReason`, so a refusal spelling that moved fails to compile here, and
// the rest of the file is exactly the run's own three.
it('spells every reason a refusal also carries as the refusal does', () => {
  const shared: PlacementReason[] = ['surfaced', 'locked', 'refused_root'];
  const runOnly: FindingReason[] = [
    'root_missing',
    'root_on_another_filesystem',
    'keyring_degraded',
  ];

  expect(
    findingReasons.filter((reason) => !(shared as string[]).includes(reason)),
  ).toEqual(runOnly);
});

// A lost key is one state on both routes: `locked` beside `KeyLost`, as the
// refusal a fetch declines it with pairs them. And the two names only a sync
// finds are in the refusals' file beside it rather than in a list of their own.
it('pairs a lost key the way a refusal does, and keeps the sync-only names in the refusals file', () => {
  const lost: Finding = {
    path: 'albums/a.jpg',
    message: 'the Library records no key for the Container holding this file',
    reason: 'locked',
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
