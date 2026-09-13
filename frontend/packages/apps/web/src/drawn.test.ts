import { afterEach, beforeEach, expect, it } from 'vitest';

import { Refusal } from '@coffret/api';

import { drawnPages, type Fetching } from './drawn';

/**
 * The browser's object URLs, as the two calls a case cares about.
 *
 * Counted rather than watched: what this module is for is that plaintext stops
 * being reachable, and a page dropped from a map without its URL being revoked
 * is a page the tab still holds. So every case here asks how many were revoked,
 * and which.
 */
const revoked: string[] = [];
let made = 0;
let createObjectURL: typeof URL.createObjectURL;
let revokeObjectURL: typeof URL.revokeObjectURL;

beforeEach(() => {
  revoked.length = 0;
  made = 0;
  createObjectURL = URL.createObjectURL;
  revokeObjectURL = URL.revokeObjectURL;
  URL.createObjectURL = () => `blob:${(made += 1)}`;
  URL.revokeObjectURL = (url: string) => {
    revoked.push(url);
  };
});

afterEach(() => {
  URL.createObjectURL = createObjectURL;
  URL.revokeObjectURL = revokeObjectURL;
});

/** Bytes standing in for a page's, since nothing here looks inside them. */
function bytes(): Blob {
  return {} as Blob;
}

/** A server that answers every page, with the paths it was asked for recorded. */
function answers(): Fetching & { asked: string[] } {
  const asked: string[] = [];
  const fetching = (path: string) => {
    asked.push(path);
    return Promise.resolve(bytes());
  };
  return Object.assign(fetching, { asked });
}

/** A page whose answer a case holds back until it says so. */
function held(): { fetching: Fetching; answer: () => void } {
  let answer = () => undefined as void;
  const fetching = () =>
    new Promise<Blob>((resolve) => {
      answer = () => resolve(bytes());
    });
  return { fetching, answer: () => answer() };
}

// The cache the reader turns pages out of: the page it already holds is the
// page it shows, without a second request for bytes this tab has in memory.
it('answers from what it holds rather than asking again', async () => {
  const fetching = answers();
  const drawn = drawnPages(fetching);

  const first = await drawn.load('albums/1.png');
  const second = await drawn.load('albums/1.png');

  expect(second).toBe(first);
  expect(drawn.held('albums/1.png')).toBe(first);
  expect(fetching.asked).toEqual(['albums/1.png']);
});

// One page asked for twice while the first ask is still out — the reader
// turning onto what it was prefetching — is one request.
it('asks once for a page two callers are waiting on', async () => {
  const fetching = answers();
  const drawn = drawnPages(fetching);

  const [first, second] = await Promise.all([
    drawn.load('albums/1.png'),
    drawn.load('albums/1.png'),
  ]);

  expect(second).toBe(first);
  expect(fetching.asked).toEqual(['albums/1.png']);
});

// The lock, and the whole of what it takes back here: the page on the screen
// and every page prefetched around it, revoked rather than only dropped.
it('revokes every page it holds when the key is given up', async () => {
  const drawn = drawnPages(answers());
  const drew = await Promise.all([
    drawn.load('albums/1.png'),
    drawn.load('albums/2.png'),
    drawn.load('albums/3.png'),
    drawn.load('albums/4.png'),
  ]);

  drawn.discard();

  expect(revoked).toEqual(drew);
  expect(drawn.held('albums/1.png')).toBeUndefined();
});

// What the reader shows after a lock. The page it was showing is not in memory
// any more, so the next turn of the screen is a request — and while the server
// is locked, what that request earns is the server's own sentence, which is
// what the reader puts up in place of the page.
it('asks the server again after a discard, and carries back what refuses it', async () => {
  let locked = false;
  const asked: string[] = [];
  const drawn = drawnPages((path) => {
    asked.push(path);
    return locked
      ? Promise.reject(new Refusal('locked', 423, 'the Passphrase is required'))
      : Promise.resolve(bytes());
  });
  await drawn.load('albums/1.png');

  drawn.discard();
  locked = true;

  await expect(drawn.load('albums/1.png')).rejects.toThrow('the Passphrase is required');
  expect(asked).toEqual(['albums/1.png', 'albums/1.png']);
});

// A page that was refused leaves nothing behind for the next ask to be answered
// out of. This is what the reader's "try again" stands on: the button reaches
// the server, rather than being handed the refusal the last one ended in.
it('asks the server again for a page that was refused', async () => {
  const asked: string[] = [];
  let refusing = true;
  const drawn = drawnPages((path) => {
    asked.push(path);
    if (refusing) {
      refusing = false;
      return Promise.reject(new Refusal('storage', 502, 'the Storage did not answer'));
    }
    return Promise.resolve(bytes());
  });

  await expect(drawn.load('albums/1.png')).rejects.toThrow('the Storage did not answer');
  const url = await drawn.load('albums/1.png');

  expect(asked).toEqual(['albums/1.png', 'albums/1.png']);
  expect(drawn.held('albums/1.png')).toBe(url);
});

// A page whose bytes were already on the wire when the lock happened. They were
// decrypted under the key the lock has just ended, so what arrives is nobody's:
// revoked as it lands, not put back into what the reader holds, and not carried
// back to the caller either — a reader handed a URL that has just been revoked
// would draw a broken page out of it.
it('revokes a page that arrives after the key was given up', async () => {
  const { fetching, answer } = held();
  const drawn = drawnPages(fetching);
  const coming = drawn.load('albums/1.png');

  drawn.discard();
  answer();

  await expect(coming).resolves.toBeUndefined();
  expect(revoked).toEqual(['blob:1']);
  expect(drawn.held('albums/1.png')).toBeUndefined();
});

// And the request that was in flight is not what the next caller is handed:
// it belongs to the key that is gone, so asking again is a new request.
it('does not answer a later caller with a request the discard let go of', async () => {
  const { fetching, answer } = held();
  const asked: string[] = [];
  const drawn = drawnPages((path) => {
    asked.push(path);
    return fetching(path);
  });
  void drawn.load('albums/1.png').catch(() => undefined);

  drawn.discard();
  void drawn.load('albums/1.png').catch(() => undefined);
  answer();

  expect(asked).toEqual(['albums/1.png', 'albums/1.png']);
});

// Two discards, with a page still on the wire from before each of them. What
// decides is which round a request was asked in and not merely that some
// discard has happened since, so an answer from two rounds back is as much
// nobody's as the one from the round just closed.
it('keeps nothing asked for in a round a discard has closed, however many have', async () => {
  const first = held();
  const second = held();
  let asking = first.fetching;
  const drawn = drawnPages((path) => asking(path));

  const older = drawn.load('albums/1.png');
  drawn.discard();
  asking = second.fetching;
  const newer = drawn.load('albums/1.png');
  drawn.discard();

  first.answer();
  second.answer();

  await expect(Promise.all([older, newer])).resolves.toEqual([undefined, undefined]);
  expect(revoked).toEqual(['blob:1', 'blob:2']);
  expect(drawn.held('albums/1.png')).toBeUndefined();
});

// A long folder read end to end would otherwise leave the tab holding every
// page of it.
it('keeps the window the reader is in and revokes the rest', async () => {
  const drawn = drawnPages(answers());
  const first = await drawn.load('albums/1.png');
  const second = await drawn.load('albums/2.png');

  drawn.keepOnly(new Set(['albums/2.png']));

  expect(revoked).toEqual([first]);
  expect(drawn.held('albums/2.png')).toBe(second);
});

// Closing the reader is this tab dropping every page it held.
it('revokes what it holds when the reader closes', async () => {
  const drawn = drawnPages(answers());
  const first = await drawn.load('albums/1.png');

  drawn.closed();

  expect(revoked).toEqual([first]);
  expect(drawn.held('albums/1.png')).toBeUndefined();
});

// Including the pages still on their way, which arrive to a reader that is not
// there to draw them.
it('revokes a page that lands after the reader closed', async () => {
  const { fetching, answer } = held();
  const drawn = drawnPages(fetching);
  const coming = drawn.load('albums/1.png');

  drawn.closed();
  answer();

  await expect(coming).resolves.toBeUndefined();
  expect(revoked).toEqual(['blob:1']);
  expect(drawn.held('albums/1.png')).toBeUndefined();
});

// Development mounts the reader twice and the pages outlive both mounts, so the
// first teardown must not leave the second refusing to keep what it fetches.
it('keeps what it fetches once the reader is here again', async () => {
  const { fetching, answer } = held();
  const drawn = drawnPages(fetching);
  const coming = drawn.load('albums/1.png');

  drawn.closed();
  drawn.reopened();
  answer();
  const url = await coming;

  expect(revoked).toEqual([]);
  expect(drawn.held('albums/1.png')).toBe(url);
});
