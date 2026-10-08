import { expect, it } from 'vitest';

import { Refusal, type LibraryState, type Unlocking } from '@coffret/api';

import { askForUnlock, lockLanded, lockUnheard, UnlockPrompting, unlockLanded } from './lock';

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

/**
 * The same tab, counting the times it asked its questions again because the
 * Library came back.
 */
function returning() {
  let held: LibraryState | null = null;
  let reloads = 0;
  return {
    answered(state: LibraryState) {
      if (unlockLanded(held, state)) {
        reloads += 1;
      }
      held = state;
    },
    get reloads() {
      return reloads;
    },
  };
}

// The way back. The Library locked under an open tab, everything it asked was
// refused, and the Passphrase was given in the desktop app's own window. The
// answer turning back to unlocked is where the tab asks everything again, so a
// page that was refused comes back without anybody pressing *try again*.
it('asks again when the answer turns from locked to unlocked', () => {
  expect(unlockLanded('locked', 'unlocked')).toBe(true);

  const back = returning();
  back.answered('unlocked');
  back.answered('locked');
  back.answered('unlocked');

  expect(back.reloads).toBe(1);
});

// A page that came up to an open Library asked its questions as it came up, and
// asking them again would be reacting to a state it was never out of.
it('asks nothing again when the first answer says unlocked', () => {
  expect(unlockLanded(null, 'unlocked')).toBe(false);

  const fresh = returning();
  fresh.answered('unlocked');
  fresh.answered('unlocked');

  expect(fresh.reloads).toBe(0);
});

// A page that came up to a shut Library is told locked first, and the unlock
// after that is news like any other: it has been refused everything so far.
it('asks again when a page that came up locked is unlocked', () => {
  const late = returning();
  late.answered('locked');
  late.answered('locked');
  late.answered('unlocked');

  expect(late.reloads).toBe(1);
  expect(unlockLanded('locked', 'locked')).toBe(false);
});

/** A press of *unlock*, recording what the screen was told. */
function press(ask: () => Promise<Unlocking>) {
  const told = { line: null as string | null, trouble: null as string | null, rechecks: 0 };
  return {
    told,
    done: askForUnlock({
      ask,
      line: (said) => {
        told.line = said;
      },
      trouble: (said) => {
        told.trouble = said;
      },
      recheck: () => {
        told.rechecks += 1;
      },
    }),
  };
}

// Inside the desktop app: the server woke the app's window, and the sentence
// says so. Nothing is unlocked yet, so nothing is asked again.
it('says the app is asking for the Passphrase', async () => {
  const pressed = press(() =>
    Promise.resolve({
      library: 'locked',
      message: 'the Coffret app is asking for the Passphrase in its own window',
    }),
  );
  await pressed.done;

  expect(pressed.told.line).toContain('its own window');
  expect(pressed.told.trouble).toBeNull();
  expect(pressed.told.rechecks).toBe(0);
});

// Under the command line there is no window, and the server says to start it
// again. That is what refused the press, and it is shown as such.
it('shows the refusal that says to start the server again', async () => {
  const sentence =
    'the Passphrase is required: this server is locked because nothing had used it for a ' +
    'while, and it is unlocked by starting it again with the Passphrase';
  const pressed = press(() => Promise.reject(new Refusal('locked', 423, sentence)));
  await pressed.done;

  expect(pressed.told.trouble).toBe(sentence);
  expect(pressed.told.line).toBeNull();
});

// Already open — the tray got there first — so the screen asks what the server
// is doing at once rather than waiting for the next tick to hear it.
it('asks what the server is doing where it is already unlocked', async () => {
  const pressed = press(() =>
    Promise.resolve({ library: 'unlocked', message: 'the Library is already unlocked' }),
  );
  await pressed.done;

  expect(pressed.told.rechecks).toBe(1);
});

/**
 * A page asking for the unlock on its own, wired the way the screen is: it
 * hears each work answer and each change of whether it is in front, and counts
 * the asks it sends.
 */
function prompted(inFront: boolean) {
  const prompting = new UnlockPrompting();
  let looking = inFront;
  let asks = 0;
  const ask = (due: boolean) => {
    if (due) {
      asks += 1;
    }
  };
  return {
    prompting,
    answered(state: LibraryState) {
      ask(prompting.heard(state, looking));
    },
    looked(now: boolean) {
      looking = now;
      ask(prompting.front(now));
    },
    get asks() {
      return asks;
    },
  };
}

// The person is at the page when the lock lands: the window comes forward at
// once, and only once however many times the answer goes on saying locked.
it('asks once for a lock seen while the page is in front', () => {
  const page = prompted(true);
  page.answered('unlocked');
  page.answered('locked');
  page.answered('locked');
  page.looked(true);

  expect(page.asks).toBe(1);
});

// A page that came up to a shut Library is owed the ask too: it is the person
// who came back to it that the ask is for.
it('asks once where the first answer already says locked', () => {
  const page = prompted(true);
  page.answered('locked');
  page.answered('locked');

  expect(page.asks).toBe(1);
});

// An idle lock lands while the person is away. Asking then would put the window
// in front of whatever they are doing elsewhere, so the ask waits for the page
// to be in front again — hidden and unfocused alike being out of it.
it('asks nothing while the page is out of front, then once when it comes back', () => {
  const page = prompted(false);
  page.answered('unlocked');
  page.answered('locked');
  page.answered('locked');
  page.looked(false);

  expect(page.asks).toBe(0);

  page.looked(true);
  page.looked(false);
  page.looked(true);

  expect(page.asks).toBe(1);
});

// The person closed the window without the Passphrase, or the ask failed: that
// lock has had its ask. The next lock, after an unlock, is owed a new one.
it('asks nothing more for a lock already asked about, and again for the next', () => {
  const page = prompted(true);
  page.answered('unlocked');
  page.answered('locked');
  page.prompting.refused(new Error('the request did not arrive'));
  page.looked(false);
  page.looked(true);
  page.answered('locked');

  expect(page.asks).toBe(1);

  page.answered('unlocked');
  page.answered('locked');

  expect(page.asks).toBe(2);
});

// A press of *unlock* pays the ask a lock is owed, so a page that comes back
// in front after one does not ask again.
it('asks nothing on its own for a lock the button was pressed for', () => {
  const page = prompted(false);
  page.answered('unlocked');
  page.answered('locked');
  page.prompting.pressed();
  page.looked(true);

  expect(page.asks).toBe(0);
});

// A server started without the desktop app has no window to bring forward, and
// its locked refusal is the only sign of that. After it, the page asks nothing
// on its own for as long as it lives; the button stays for a person to press.
it('asks nothing on its own after the server says it has no window', () => {
  const page = prompted(true);
  page.answered('locked');

  expect(page.asks).toBe(1);

  page.prompting.refused(new Refusal('locked', 423, 'start it again with the Passphrase'));
  page.answered('unlocked');
  page.answered('locked');
  page.looked(false);
  page.looked(true);

  expect(page.asks).toBe(1);
});

// Only the locked refusal says there is no window. A request that never reached
// the server, or one refused for anything else, leaves the next lock asked for
// as a desktop app's server would want it.
it('asks again for the next lock after a refusal that is not the locked one', () => {
  const page = prompted(true);
  page.answered('locked');

  expect(page.asks).toBe(1);

  page.prompting.refused(new TypeError('Failed to fetch'));
  page.prompting.refused(new Refusal('server', 500, 'something went wrong'));
  page.answered('unlocked');
  page.answered('locked');

  expect(page.asks).toBe(2);
});

// A page with the reader closed asks nothing at the interval, so the lock
// reaches it first as a listing refused with the locked refusal. Where it
// believes the Library open, that refusal sends it to read the work answer.
it('reads the work answer again on a locked refusal it believes open', () => {
  expect(lockUnheard('unlocked', true)).toBe(true);
});

// Where the page already knows it is locked, or has not been told yet, the work
// answer is already what it goes by; and only the locked refusal sends it.
it('reads the work answer for no other refusal and no other state', () => {
  expect(lockUnheard('locked', true)).toBe(false);
  expect(lockUnheard(null, true)).toBe(false);
  expect(lockUnheard('unlocked', false)).toBe(false);
});
