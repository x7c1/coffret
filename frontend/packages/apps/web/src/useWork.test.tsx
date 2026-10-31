import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import {
  getWork,
  Refusal,
  startFill,
  type Fill,
  type FillStatus,
  type NotStopped,
  type Refused,
  type Stopped,
  type Work,
} from '@coffret/api';

import { POLL_INTERVAL_MS } from './fill';
import { useWork } from './useWork';

// The server, as this hook reaches it. Everything else the package exports is
// the real thing — `Refusal` above all, since what the hook keeps of a refused
// press is read off one.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getWork: vi.fn(),
  startFill: vi.fn(),
  startSync: vi.fn(),
  startFreeze: vi.fn(),
}));

const asked = vi.mocked(getWork);
const filled = vi.mocked(startFill);

/** What stopped the fills in these cases: Storage not answering. */
const STORAGE: Refused = {
  kind: 'storage',
  message: "the Library's Storage did not answer",
  reason: null,
  surfaced: null,
};

/**
 * A fill of `albums`, stopped by Storage unless `standing` names another state —
 * and a status and its refusal only ever in one of the pairs they come in.
 */
function aFill(
  over: Partial<Omit<Fill, 'status' | 'stopped'>> = {},
  standing: NotStopped<Exclude<FillStatus, 'stopped'>> | Stopped = {
    status: 'stopped',
    stopped: STORAGE,
  },
): Fill {
  return {
    run: 1,
    folder: 'albums',
    total: 2,
    done: 0,
    declined: [],
    findings: [],
    waiting: [],
    discarded: [],
    displaced: [],
    ...over,
    ...standing,
  };
}

/**
 * The work answer with `fill` as the latest fill, and in the log as well where
 * it is over — which is where the server puts a run as it ends (spec: LA-12).
 */
function answer(fill: Fill | null): Work {
  const over = fill !== null && fill.status !== 'filling';
  return {
    server: 'one-process',
    library: 'unlocked',
    catalog: { state: 'caught_up', stopped: null },
    turn: null,
    fill,
    sync: null,
    freeze: null,
    delete: null,
    reconnect: null,
    log: over ? [{ id: fill.run, at: '2026-10-30T09:00:00Z', settled: false, kind: 'fill', run: fill }] : [],
  };
}

/** Lets the interval tick `times` times, and every answer it asked for land. */
async function ticks(times = 1): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * times);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
  asked.mockReset();
  filled.mockReset();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

// What a press of "bring over again" was refused with stays on the screen for
// as long as the Retry that met it does — and the next poll is not that. The
// poll answers with the very entry the Retry is offered from, so the refusal is
// still about something a person can see; taking it away there would leave the
// button standing and nothing saying why pressing it did nothing. What does end
// it is the offer ending: the entry settled by a later run of the same folder,
// which here is the folder being brought over after all.
it('keeps a refused press across a poll and lets it go once the offer ends', async () => {
  asked.mockResolvedValue(answer(aFill()));
  // A reader open, so that the page is polling rather than having asked once.
  const { result } = renderHook(() => useWork(true));
  await ticks(0);
  expect(result.current.fill?.status).toBe('stopped');

  filled.mockRejectedValue(new Refusal('locked', 423, 'the Passphrase is required'));
  await act(async () => {
    result.current.retry('albums');
    await vi.advanceTimersByTimeAsync(0);
  });
  const refused = {
    pressed: { flow: 'fill', folder: 'albums' },
    said: 'the Passphrase is required',
    ranOut: false,
  };
  expect(result.current.trouble).toEqual(refused);

  const before = asked.mock.calls.length;
  await ticks();
  expect(asked.mock.calls.length, 'the interval asked again').toBeGreaterThan(before);
  expect(result.current.trouble, 'and the answer it had did not end the offer').toEqual(refused);

  asked.mockResolvedValue(answer(aFill({ done: 2 }, { status: 'done', stopped: null })));
  await ticks();
  expect(result.current.fill?.status).toBe('done');
  expect(result.current.trouble, 'the button is gone, and so is what it met').toBeNull();
});

// A refusal of a press whose entry is no longer offering it is about nothing
// a person can look at: an entry settled by the run after it offers no Retry.
it('lets a refused press go once its entry is settled', async () => {
  asked.mockResolvedValue(answer(aFill()));
  const { result } = renderHook(() => useWork(true));
  await ticks(0);

  filled.mockRejectedValue(new Refusal('storage', 502, "the Library's Storage did not answer"));
  await act(async () => {
    result.current.retry('albums');
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(result.current.trouble).not.toBeNull();

  const settled = answer(aFill());
  asked.mockResolvedValue({
    ...settled,
    log: settled.log.map((entry) => ({ ...entry, settled: true })),
  });
  await ticks();
  expect(result.current.trouble).toBeNull();
});
