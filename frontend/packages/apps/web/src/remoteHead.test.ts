import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import type { HeadState, RemoteHead } from '@coffret/api';

import { LOOK_EVERY_MS, RemoteHeadLooks } from './remoteHead';

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

/** The looks, over a server that answers `head`, with everything recorded. */
function looks(head: HeadState = 'newer') {
  const state = { held: false, shown: true };
  const heard: HeadState[] = [];
  const ask = vi.fn((): Promise<RemoteHead> => Promise.resolve({ head }));
  const watch = new RemoteHeadLooks({
    ask,
    heard: (said) => heard.push(said),
    held: () => state.held,
    shown: () => state.shown,
  });
  return { state, heard, ask, watch };
}

/** Lets the answers already given reach the screen. */
async function settled(): Promise<void> {
  await vi.advanceTimersByTimeAsync(0);
}

// As the page comes up: one look, and what it found is drawn.
it('looks once on load', async () => {
  const { ask, heard, watch } = looks('newer');

  const stop = watch.start();
  await settled();

  expect(ask).toHaveBeenCalledTimes(1);
  expect(heard).toEqual(['newer']);
  stop();
});

// Coming back to the tab is when somebody would glance at the icon, so it is
// looked at then rather than at the next tick.
it('looks again when the tab comes back to the front', async () => {
  const { ask, watch } = looks('current');
  const stop = watch.start();
  await settled();

  watch.front();
  await settled();

  expect(ask).toHaveBeenCalledTimes(2);
  stop();
});

// And every few minutes while the tab is shown — not while it is hidden, when
// nobody would see the answer.
it('looks on its interval while the tab is shown, and not while it is hidden', async () => {
  const { state, ask, watch } = looks('current');
  const stop = watch.start();
  await settled();

  await vi.advanceTimersByTimeAsync(LOOK_EVERY_MS);
  expect(ask).toHaveBeenCalledTimes(2);

  state.shown = false;
  await vi.advanceTimersByTimeAsync(LOOK_EVERY_MS * 3);
  expect(ask).toHaveBeenCalledTimes(2);

  state.shown = true;
  await vi.advanceTimersByTimeAsync(LOOK_EVERY_MS);
  expect(ask).toHaveBeenCalledTimes(3);

  stop();
  await vi.advanceTimersByTimeAsync(LOOK_EVERY_MS * 3);
  expect(ask).toHaveBeenCalledTimes(3);
});

// Never while a catch-up runs: on load, on the way back to the front, or on the
// interval.
it('does not look while a catch-up is running', async () => {
  const { state, ask, watch } = looks('newer');
  state.held = true;

  const stop = watch.start();
  watch.front();
  await vi.advanceTimersByTimeAsync(LOOK_EVERY_MS * 2);

  expect(ask).not.toHaveBeenCalled();
  stop();
});

// A look already on its way when a catch-up begins answers about the catalog
// before it: drawn afterwards it would say changes are waiting that have just
// been taken in, so it is not drawn.
it('drops an answer to a look that was asked before a catch-up began', async () => {
  let answer: (head: RemoteHead) => void = () => undefined;
  const heard: HeadState[] = [];
  const watch = new RemoteHeadLooks({
    ask: () =>
      new Promise<RemoteHead>((resolve) => {
        answer = resolve;
      }),
    heard: (said) => heard.push(said),
    held: () => false,
    shown: () => true,
  });

  watch.look();
  watch.forget();
  answer({ head: 'newer' });
  await settled();

  expect(heard).toEqual([]);
});

// A look that fails is not news: whatever was drawn stays, and the next look
// tries again.
it('keeps what it last heard when a look fails, and looks again next time', async () => {
  const heard: HeadState[] = [];
  const ask = vi
    .fn<() => Promise<RemoteHead>>()
    .mockResolvedValueOnce({ head: 'newer' })
    .mockRejectedValueOnce(new Error("the Library's Storage did not answer"))
    .mockResolvedValueOnce({ head: 'current' });
  const watch = new RemoteHeadLooks({
    ask,
    heard: (said) => heard.push(said),
    held: () => false,
    shown: () => true,
  });

  watch.look();
  await settled();
  watch.look();
  await settled();
  expect(heard).toEqual(['newer']);

  watch.look();
  await settled();
  expect(heard).toEqual(['newer', 'current']);
});
