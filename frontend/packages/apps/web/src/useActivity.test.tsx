import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { getActivity, Refusal, startFill, type Activity, type Fill } from '@coffret/api';

import { ACTIVITY_INTERVAL_MS } from './fill';
import { useActivity } from './useActivity';

// The server, as this hook reaches it. Everything else the package exports is
// the real thing — `Refusal` above all, since what the hook keeps of a refused
// press is read off one.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getActivity: vi.fn(),
  startFill: vi.fn(),
  startSync: vi.fn(),
  startFreeze: vi.fn(),
}));

const asked = vi.mocked(getActivity);
const filled = vi.mocked(startFill);

function aFill(over: Partial<Fill> = {}): Fill {
  return {
    run: 1,
    folder: 'albums',
    status: 'stopped',
    total: 2,
    done: 0,
    declined: [],
    waiting: [],
    dropped: [],
    displaced: [],
    stopped: { error: 'storage', message: "the Library's Storage did not answer" },
    ...over,
  };
}

function answer(fill: Fill | null): Activity {
  return {
    server: 'one-process',
    library: 'unlocked',
    catalog: { state: 'caught_up', trouble: null },
    fill,
    sync: null,
    freeze: null,
  };
}

/** Lets the interval tick `times` times, and every answer it asked for land. */
async function ticks(times = 1): Promise<void> {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ACTIVITY_INTERVAL_MS * times);
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
// as long as the button that met it does — and the next poll is not that. The
// poll answers with the very run the button is offered from, so the refusal is
// still about something a person can see; taking it away there would leave the
// button standing and nothing saying why pressing it did nothing. What does end
// it is the offer ending: the run leaving the state the button was offered
// from, which here is the folder being brought over after all.
it('keeps a refused press across a poll and lets it go once the offer ends', async () => {
  asked.mockResolvedValue(answer(aFill()));
  // A reader open, so that the page is polling rather than having asked once.
  const { result } = renderHook(() => useActivity(true));
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
  };
  expect(result.current.trouble).toEqual(refused);

  const before = asked.mock.calls.length;
  await ticks();
  expect(asked.mock.calls.length, 'the interval asked again').toBeGreaterThan(before);
  expect(result.current.trouble, 'and the answer it had did not end the offer').toEqual(refused);

  asked.mockResolvedValue(answer(aFill({ status: 'done', done: 2, stopped: null })));
  await ticks();
  expect(result.current.fill?.status).toBe('done');
  expect(result.current.trouble, 'the button is gone, and so is what it met').toBeNull();
});

// And the other road an offer ends by: the notice it stood beside put away.
// A refusal of a button that is no longer drawn is about nothing a person can
// look at.
it('lets a refused press go once its notice is put away', async () => {
  asked.mockResolvedValue(answer(aFill()));
  const { result } = renderHook(() => useActivity(true));
  await ticks(0);

  filled.mockRejectedValue(new Refusal('storage', 502, "the Library's Storage did not answer"));
  await act(async () => {
    result.current.retry('albums');
    await vi.advanceTimersByTimeAsync(0);
  });
  expect(result.current.trouble).not.toBeNull();

  act(() => result.current.dismiss({ kind: 'line', flow: 'fill' }));
  expect(result.current.trouble).toBeNull();

  // Nor does it come back when the next answer brings the same run.
  await ticks();
  expect(result.current.trouble).toBeNull();
});
