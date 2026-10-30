import { expect, it } from 'vitest';

import type { DisplacedFill, Fill, Freeze, Refused, Sync } from '@coffret/api';

import { retryable } from './retry';

/** What stops a run in these cases unless a case says otherwise. */
const STORAGE: Refused = {
  kind: 'storage',
  message: 'Storage did not answer',
  reason: null,
  surfaced: null,
};

function aFill(stopped: Refused | null = STORAGE): Fill {
  const own = {
    run: 1,
    folder: 'albums',
    total: 2,
    done: 0,
    declined: [],
    findings: [],
    waiting: [],
    discarded: [],
    displaced: [],
  };
  return stopped === null
    ? { ...own, status: 'filling', stopped: null }
    : { ...own, status: 'stopped', stopped };
}

function displacedFill(stopped: Refused = STORAGE): DisplacedFill {
  return {
    run: 1,
    folder: 'albums',
    total: 2,
    done: 0,
    declined: [],
    findings: [],
    status: 'stopped',
    stopped,
  };
}

function aSync(stopped: Refused = STORAGE): Sync {
  return { run: 1, status: 'stopped', added: 0, findings: [], step: null, waits: false, stopped };
}

function aFreeze(stopped: Refused = STORAGE): Freeze {
  return {
    run: 1,
    folder: 'books/vol-1',
    status: 'stopped',
    packs: 0,
    entries: 0,
    findings: [],
    step: null,
    waits: false,
    waiting: [],
    discarded: [],
    displaced: [],
    stopped,
  };
}

// Storage going away is the second attempt's own case: once it is back, the
// same request does what the first one could not.
it('offers a second attempt at a run Storage stopped', () => {
  expect(retryable(aFill())).toBe(true);
  expect(retryable(aSync())).toBe(true);
  expect(retryable(aFreeze())).toBe(true);
  expect(retryable(displacedFill())).toBe(true);
});

// Only a run that stopped is offered one: a run under way is not asked for
// again, and no run at all has nothing to ask.
it('offers none at a run that did not stop', () => {
  expect(retryable(aFill(null))).toBe(false);
  expect(retryable(null)).toBe(false);
});

// A refused mapped root is mended at a terminal, by the gesture its own
// explanation names, and asking again meets the same mapping.
it('offers none at a run a refused root stopped', () => {
  const root: Refused = {
    kind: 'refused_placement',
    message: 'the mapping is not the one recorded',
    reason: 'refused_root',
    surfaced: null,
  };
  expect(retryable(aFill(root))).toBe(false);
  expect(retryable(aSync(root))).toBe(false);
  expect(retryable(aFreeze(root))).toBe(false);
  expect(retryable(displacedFill(root))).toBe(false);
});

// `epoch`, `library_predates_format` and `locked` are about this device or the
// Library rather than the run, so asking again meets the same refusal.
it('offers none at a run stopped by an epoch, an old format or a lock', () => {
  for (const kind of ['epoch', 'library_predates_format', 'locked'] as const) {
    const stopped: Refused = {
      kind,
      message: 'what remedies this is elsewhere',
      reason: null,
      surfaced: null,
    };
    expect(retryable(aFill(stopped)), kind).toBe(false);
    expect(retryable(aSync(stopped)), kind).toBe(false);
    expect(retryable(aFreeze(stopped)), kind).toBe(false);
  }
});
