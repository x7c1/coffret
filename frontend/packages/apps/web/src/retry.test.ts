import { expect, it } from 'vitest';

import type { Fill, Freeze, Sync } from '@coffret/api';

import { NOTHING_DISMISSED, putAway, putAwayFolders } from './dismissed';
import {
  offeredAgain,
  offeredFolders,
  offersAgain,
  retryable,
  stillStanding,
  type Trouble,
} from './retry';

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
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function aSync(over: Partial<Sync> = {}): Sync {
  return {
    run: 1,
    status: 'stopped',
    added: 0,
    findings: [],
    step: null,
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function aFreeze(over: Partial<Freeze> = {}): Freeze {
  return {
    run: 1,
    folder: 'books/vol-1',
    status: 'stopped',
    packs: 0,
    entries: 0,
    findings: [],
    step: null,
    waiting: [],
    dropped: [],
    displaced: [],
    stopped: { error: 'storage', message: 'Storage did not answer' },
    ...over,
  };
}

function refused(pressed: Trouble['pressed']): Trouble {
  return { pressed, said: 'the Library is locked on this device' };
}

// The failure this is here for. The folders a queue lost are not a run and have
// no status, so nothing about the three runs says whether that offer is still
// being made — and a refusal ended by the runs went off the screen at the next
// poll, under a second, with the button that met it still standing.
it('keeps a lost folder’s refusal for as long as that folder is still offered', () => {
  const trouble = refused({ flow: 'fill', folder: 'books' });
  const lost = aFill({ status: 'filling', stopped: null, dropped: ['books'] });

  expect(stillStanding(trouble, lost, null, null, NOTHING_DISMISSED)).toBe(trouble);

  // Taken up — by this button landing at last, or by somebody opening a file in
  // it — is what ends the offer, and with it the reason it was refused.
  const taken = aFill({ status: 'filling', stopped: null, dropped: [] });
  expect(stillStanding(trouble, taken, null, null, NOTHING_DISMISSED)).toBeNull();
});

// And the same for a run the next one took the record from. It is not the run on
// record, so nothing the run on record says ends its offer — what ends it is
// somebody taking that folder up, exactly as for a folder the queue lost.
it('keeps a displaced run’s refusal for as long as that run is still offered', () => {
  const trouble = refused({ flow: 'fill', folder: 'albums' });
  const running = aFill({
    folder: 'letters',
    status: 'filling',
    stopped: null,
    displaced: [aFill({ folder: 'albums' })],
  });

  expect(stillStanding(trouble, running, null, null, NOTHING_DISMISSED)).toBe(trouble);
  expect(offeredFolders(running)).toEqual(['albums']);

  const taken = aFill({ folder: 'albums', status: 'filling', stopped: null });
  expect(stillStanding(trouble, taken, null, null, NOTHING_DISMISSED)).toBeNull();

  // And the notice being put away ends it too, because the button goes with the
  // notice: a sentence under no button is a person told that something they
  // cannot see was refused.
  const forgot = putAwayFolders(NOTHING_DISMISSED, 'fill', ['albums']);
  expect(stillStanding(trouble, running, null, null, forgot)).toBeNull();
});

// Both ways a flow offers a folder by name, read together: what ends either
// offer is the same thing, so the question is asked in one place.
it('names the folders a flow offers by either of the two ways it offers one', () => {
  expect(offeredFolders(null)).toEqual([]);
  const both = aFreeze({
    folder: 'books/vol-3',
    status: 'freezing',
    stopped: null,
    dropped: ['books/vol-2'],
    displaced: [aFreeze({ folder: 'books/vol-1' })],
  });

  expect(offeredFolders(both)).toEqual(['books/vol-2', 'books/vol-1']);
});

it('does not let one flow take another flow’s refusal off the screen', () => {
  const trouble = refused({ flow: 'freeze', folder: 'books/vol-1' });
  // A fill running says nothing about the freeze that refused "pack again", and
  // the run it was offered from is still stopped.
  const running = aFill({ status: 'filling', stopped: null });

  expect(stillStanding(trouble, running, null, aFreeze(), NOTHING_DISMISSED)).toBe(
    trouble,
  );
});

it('ends a refusal once its own run leaves the state it was offered from', () => {
  const trouble = refused({ flow: 'sync' });
  const again = aSync({ status: 'syncing', stopped: null });

  expect(stillStanding(trouble, null, aSync(), null, NOTHING_DISMISSED)).toBe(trouble);
  expect(stillStanding(trouble, null, again, null, NOTHING_DISMISSED)).toBeNull();
});

// The offer and the refusal it met are one notice. A line put away takes its
// button with it, so a sentence left under no button would be a person told
// that something they can no longer see was refused.
it('takes a refusal away with the notice it was standing under', () => {
  const line = refused({ flow: 'fill', folder: 'albums' });
  const read = putAway(NOTHING_DISMISSED, 'fill', 1);
  expect(stillStanding(line, aFill(), null, null, read)).toBeNull();

  const lost = aFill({ status: 'filling', stopped: null, dropped: ['books'] });
  const folder = refused({ flow: 'fill', folder: 'books' });
  const forgot = putAwayFolders(NOTHING_DISMISSED, 'fill', ['books']);
  expect(stillStanding(folder, lost, null, null, forgot)).toBeNull();
});

// A run whose explanation names the only recovery there is offers no second
// attempt, so nothing about it can have been refused.
it('stands under no offer where a refused root left none', () => {
  const trouble = refused({ flow: 'fill', folder: 'albums' });
  const refusedRoot = aFill({
    stopped: {
      error: 'declined',
      message: 'the mapping is not the one recorded',
      reason: 'refused_root',
    },
  });

  expect(stillStanding(trouble, refusedRoot, null, null, NOTHING_DISMISSED)).toBeNull();
});

// A displaced run is a run whose failure nothing has happened to: the next
// folder starting is somebody else's doing. So the question of whether pressing
// could change the answer is asked of it exactly as it is asked of the run on
// record — and a refused mapped root, which only `coffret map` settles, is
// offered no button under either.
it('offers a second attempt at the displaced runs repeating could help', () => {
  const mapping = {
    error: 'declined' as const,
    message: 'the mapping is not the one recorded',
    reason: 'refused_root' as const,
  };
  const stopped = aFill({ folder: 'albums' });
  const refusedRoot = aFill({ folder: 'letters', stopped: mapping });

  expect(offeredAgain([stopped, refusedRoot])).toEqual([stopped]);

  // The notice is the wider of the two and keeps both: the folder is half here
  // whatever refused it, and the line saying so is what the explanation naming
  // `coffret map` reaches a person through.
  const running = aFill({
    folder: 'books',
    status: 'filling',
    stopped: null,
    displaced: [stopped, refusedRoot],
  });
  expect(offeredFolders(running)).toEqual(['albums', 'letters']);

  // And no button is no refusal: the offer a red line would answer was never
  // made, so a sentence naming it would stand under nothing.
  expect(
    stillStanding(
      refused({ flow: 'fill', folder: 'albums' }),
      running,
      null,
      null,
      NOTHING_DISMISSED,
    ),
  ).not.toBeNull();
  expect(
    stillStanding(
      refused({ flow: 'fill', folder: 'letters' }),
      running,
      null,
      null,
      NOTHING_DISMISSED,
    ),
  ).toBeNull();
});

// The rule the two readers share: how long a refusal of that folder lives, and
// which button's words the bar names it by. One press reaches both, and they
// disagree the moment either grows a reading of its own.
it('tells a stopped run’s second attempt from a folder its queue lost', () => {
  const stopped = aFill({ dropped: ['letters'] });
  expect(offersAgain(stopped, 'albums')).toBe(true);
  expect(offersAgain(stopped, 'letters')).toBe(false);

  // No second attempt at all is no second attempt at its own folder either,
  // which is where the two would part first: the lifetime rule already ends a
  // refused root's refusal, and a bar reading it apart would go on calling the
  // press "bring over again" under no such button.
  const refusedRoot = aFill({
    stopped: {
      error: 'declined',
      message: 'the mapping is not the one recorded',
      reason: 'refused_root',
    },
  });
  expect(offersAgain(refusedRoot, 'albums')).toBe(false);
  expect(offersAgain(aFill({ status: 'filling', stopped: null }), 'albums')).toBe(false);
  expect(offersAgain(null, 'albums')).toBe(false);
});

// `epoch` and `locked` are about this device rather than the run, so asking
// again meets the same refusal (see `retryable`). No button is offered for a
// stopped run, for one a later run took the record from, or for the sync — and
// with no button, an earlier refusal of a press is not kept standing either.
// Storage going away is the retry's own case, and keeps it.
it('offers no second attempt at a run stopped by an epoch or a lock', () => {
  for (const error of ['epoch', 'locked'] as const) {
    const stopped = { error, message: 'what settles this is at a terminal' };

    expect(retryable(aFill({ stopped })), error).toBe(false);
    expect(retryable(aSync({ stopped })), error).toBe(false);
    expect(retryable(aFreeze({ stopped })), error).toBe(false);
    expect(offeredAgain([aFill({ folder: 'books', stopped })]), error).toEqual([]);

    const fill = aFill({ stopped });
    expect(
      stillStanding(refused({ flow: 'fill', folder: 'albums' }), fill, null, null, NOTHING_DISMISSED),
      error,
    ).toBeNull();
    expect(
      stillStanding(refused({ flow: 'sync' }), null, aSync({ stopped }), null, NOTHING_DISMISSED),
      error,
    ).toBeNull();
  }

  expect(retryable(aFill()), 'Storage that did not answer').toBe(true);
  expect(retryable(aSync()), 'Storage that did not answer').toBe(true);
  expect(retryable(aFreeze()), 'Storage that did not answer').toBe(true);
});
