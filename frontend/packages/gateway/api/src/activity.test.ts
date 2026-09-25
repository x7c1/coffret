import { expect, it } from 'vitest';

import type { Finding, FindingReason, Sync } from './activity';
import findingReasons from './finding-reasons.json';
import type { DeclinedReason, SurfacedFinding } from './refusal';
import surfacedFindings from './surfaced-findings.json';

// The shape a page reads a finding in, as the server's route test pins it on
// the wire: the Entry, the sentence, and which finding it is in the two fields
// a declined fetch names one by. Written as a typed literal so that the
// compiler holds it to the interface — a field renamed or dropped on one side
// fails here — and the keys are asserted so that a field added on this side
// alone fails too.
it('reads a finding as its Entry, its sentence, its reason and what it surfaced', () => {
  const sync: Sync = {
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
  };
  const [file, root] = sync.findings;

  expect(Object.keys(file).sort()).toEqual(['message', 'path', 'reason', 'surfaced']);
  expect(Object.keys(root).sort()).toEqual(['message', 'path', 'reason']);
  expect(root.surfaced).toBeUndefined();
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
  ];

  expect(reasons).toEqual(findingReasons);
});

// One state, one spelling: every finding reason a refusal can also carry is
// the refusal's own literal. The ones a finding shares are typed as
// `DeclinedReason`, so a refusal spelling that moved fails to compile here, and
// the rest of the file is exactly the run's own two.
it('spells every reason a refusal also carries as the refusal does', () => {
  const shared: DeclinedReason[] = ['surfaced', 'locked', 'refused_root'];
  const runOnly: FindingReason[] = ['root_missing', 'root_on_another_filesystem'];

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
