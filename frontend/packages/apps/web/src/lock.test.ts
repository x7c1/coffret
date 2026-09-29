import { expect, it } from 'vitest';

import type { LibraryState } from '@coffret/api';

import { lockLanded } from './lock';

/**
 * A tab reading one work answer after another, wired the way the screen is.
 *
 * It holds the state it was last told and counts the times it gave up what this
 * device decrypted.
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
