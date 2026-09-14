import { expect, it, vi } from 'vitest';

import { Refusal, type LibraryState, type Locked } from '@coffret/api';

import { askToLock, lockLanded } from './lock';

/** One lock, with everything it reaches out to recorded in the order it went. */
function locking(ask: () => Promise<Locked>) {
  const order: string[] = [];
  const trouble: (string | null)[] = [];
  return {
    order,
    trouble,
    run: () =>
      askToLock({
        ask,
        discard: () => order.push('discard'),
        reload: () => order.push('reload'),
        trouble: (line) => trouble.push(line),
      }),
  };
}

// The whole of what the gesture does: the pages this device decrypted are given
// up, and then the screen asks its questions again — which is what puts the
// server's own sentence about the Passphrase where the rows were.
//
// The discard comes first, and the order is the point: the plaintext is gone
// the moment the key is, rather than when a request the network is still
// carrying comes back.
it('gives up what this device decrypted and then asks the screen again', async () => {
  const ask = vi.fn(() => Promise.resolve({ locked: true }));
  const run = locking(ask);

  await run.run();

  expect(ask).toHaveBeenCalledTimes(1);
  expect(run.order).toEqual(['discard', 'reload']);
  expect(run.trouble).toEqual([null]);
});

// A lock that did not happen takes nothing back. The pages are still protected
// by a key the server still holds, so the reading somebody is in the middle of
// is left exactly as it was — and the sentence says outright that the Library
// is open, because one who read only the reason could walk away from a machine
// they believe is closed.
it('revokes nothing where the lock was refused, and says the Library is open', async () => {
  const run = locking(() =>
    Promise.reject(
      new Refusal('storage', 502, "the Library's Storage did not answer"),
    ),
  );

  await run.run();

  expect(run.order).toEqual([]);
  expect(run.trouble).toEqual([
    null,
    "the Library is still open on this device — the Library's Storage did not answer",
  ]);
});

// The press after that one. The sentence a refused lock leaves says the Library
// is open, and it is the one sentence on this screen somebody could act on by
// walking away — so a lock that takes has to take it down with it, or the screen
// would say the Library is open in the notice and refuse everything below it
// because it is shut.
it('takes down the sentence about an open Library when a later lock takes', async () => {
  let asked = 0;
  const run = locking((): Promise<Locked> => {
    asked += 1;
    return asked === 1
      ? Promise.reject(new Refusal('storage', 502, "the Library's Storage did not answer"))
      : Promise.resolve({ locked: true });
  });

  await run.run();
  await run.run();

  expect(run.order).toEqual(['discard', 'reload']);
  expect(run.trouble.at(-1)).toBeNull();
});

/**
 * A tab reading one activity answer after another, wired the way the screen is.
 *
 * It holds the state it was last told and counts the times it gave up what this
 * device decrypted. `pressed` is the gesture made in this same tab: it gives the
 * pages up itself, the moment the server answers, and records the state it put
 * the server into.
 */
function tab() {
  let held: LibraryState | null = null;
  let discards = 0;
  return {
    answered(state: LibraryState) {
      if (lockLanded(held, state)) {
        discards += 1;
      }
      held = state;
    },
    pressed() {
      discards += 1;
      held = 'locked';
    },
    get discards() {
      return discards;
    },
  };
}

// The case this exists for. A window was left open on a page, the server's own
// clock ran out, and nothing asked it anything — so the only thing that changes
// is the answer about what it is doing. That change is where the page lets go of
// the plaintext it is holding, rather than at the next page turn.
it('gives up what it decrypted when the answer turns from unlocked to locked', () => {
  expect(lockLanded('unlocked', 'locked')).toBe(true);

  const open = tab();
  open.answered('unlocked');
  open.answered('locked');

  expect(open.discards).toBe(1);
});

// A page that came up to a Library already shut. Every question it asked was
// refused, there is no page on the screen and nothing decrypted behind it, and a
// discard here would be this screen reacting to a state it was never in.
it('discards nothing when the first answer already says locked', () => {
  expect(lockLanded(null, 'locked')).toBe(false);

  const late = tab();
  late.answered('locked');

  expect(late.discards).toBe(0);
});

// And then it goes on saying so, several times a second for as long as the tab
// is open. Only the change is news: a page that acted on every answer would be a
// reader throwing away and re-asking for a page it is refused, over and over.
it('discards nothing when locked follows locked', () => {
  expect(lockLanded('locked', 'locked')).toBe(false);

  const shut = tab();
  shut.answered('unlocked');
  shut.answered('locked');
  shut.answered('locked');
  shut.answered('locked');

  expect(shut.discards).toBe(1);
});

// An open Library answering that it is open is not news either, however many
// times it says it.
it('discards nothing while the Library stays open', () => {
  expect(lockLanded(null, 'unlocked')).toBe(false);
  expect(lockLanded('unlocked', 'unlocked')).toBe(false);

  const reading = tab();
  reading.answered('unlocked');
  reading.answered('unlocked');

  expect(reading.discards).toBe(0);
});

// The lock this tab asked for, coming back round as an answer. The gesture gave
// the pages up itself — it must, since the plaintext is gone when the key is
// rather than when the next poll lands — and the answer that follows says what
// this page already acted on. Once, not twice.
it('does not give the pages up twice when this tab asked for the lock', () => {
  const pressing = tab();
  pressing.answered('unlocked');
  pressing.pressed();
  pressing.answered('locked');
  pressing.answered('locked');

  expect(pressing.discards).toBe(1);
});
