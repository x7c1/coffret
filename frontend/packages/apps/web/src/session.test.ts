import { afterEach, expect, it, vi } from 'vitest';

import { dropToken, heldToken, Refusal, type Launched, type SessionStanding } from '@coffret/api';

import { establish, KEPT_PREFIX, shelf, takeTicket, type Asking } from './session';

afterEach(() => {
  dropToken();
  window.sessionStorage.clear();
});

const TICKET = 't'.repeat(43);
const TOKEN = 'k'.repeat(43);

/** A host that exchanges every ticket and admits only `admitted`. */
function host(admitted: string[] = []): Asking & {
  launch: ReturnType<typeof vi.fn>;
  checkSession: ReturnType<typeof vi.fn>;
} {
  return {
    launch: vi.fn(
      (): Promise<Launched> =>
        Promise.resolve({ token: TOKEN, expires_in: 43_200, instance_id: 'host.1' }),
    ),
    checkSession: vi.fn((token: string): Promise<SessionStanding> =>
      admitted.includes(token)
        ? Promise.resolve({ expires_in: 600, library: 'unlocked' })
        : Promise.reject(new Refusal('authentication_required', 401, 'no session')),
    ),
  };
}

// LA-21: the ticket is taken out of the fragment and the fragment out of the
// address before anything else, so no history entry, bookmark or copied link
// carries it.
it('takes the ticket out of the address at once', () => {
  const history = { replaceState: vi.fn() };
  const ticket = takeTicket(
    { hash: `#launch=${TICKET}`, pathname: '/', search: '' },
    history,
  );
  expect(ticket).toBe(TICKET);
  expect(history.replaceState).toHaveBeenCalledTimes(1);
  expect(history.replaceState).toHaveBeenCalledWith(null, '', '/');

  // An address with no ticket in it is left as it is.
  const untouched = { replaceState: vi.fn() };
  expect(takeTicket({ hash: '#path=albums', pathname: '/', search: '' }, untouched)).toBeNull();
  expect(untouched.replaceState).not.toHaveBeenCalled();
});

// LA-16, LA-17: a ticket is exchanged once, however many wait on the answer,
// and the token is held and kept under the generation it belongs to.
it('exchanges the ticket once, and keeps the token for this tab', async () => {
  const asking = host();
  const shelved = shelf();

  const opened = establish(TICKET, shelved, asking);
  await Promise.all([opened, opened]);

  expect(await opened).toEqual({ kind: 'held', expiresIn: 43_200 });
  expect(asking.launch).toHaveBeenCalledTimes(1);
  expect(asking.launch).toHaveBeenCalledWith(TICKET);
  expect(asking.checkSession).not.toHaveBeenCalled();
  expect(heldToken()).toBe(TOKEN);
  expect(window.sessionStorage.getItem(`${KEPT_PREFIX}host.1`)).toBe(TOKEN);
  expect(window.localStorage.length).toBe(0);
});

// A token kept from before is used only once the host says it still stands.
it('uses a kept token only once the host accepts it', async () => {
  window.sessionStorage.setItem(`${KEPT_PREFIX}host.0`, 'gone'.padEnd(43, '-'));
  window.sessionStorage.setItem(`${KEPT_PREFIX}host.1`, TOKEN);
  const asking = host([TOKEN]);

  const opened = await establish(null, shelf(), asking);

  expect(opened).toEqual({ kind: 'held', expiresIn: 600 });
  expect(asking.launch).not.toHaveBeenCalled();
  expect(heldToken()).toBe(TOKEN);
  expect(window.sessionStorage.getItem(`${KEPT_PREFIX}host.0`)).toBeNull();
  expect(window.sessionStorage.getItem(`${KEPT_PREFIX}host.1`)).toBe(TOKEN);
});

// One the host refuses is forgotten, and nothing is held.
it('forgets a kept token the host refuses, and holds nothing', async () => {
  window.sessionStorage.setItem(`${KEPT_PREFIX}host.0`, TOKEN);

  const opened = await establish(null, shelf(), host());

  expect(opened).toEqual({ kind: 'none', said: null });
  expect(heldToken()).toBeNull();
  expect(window.sessionStorage.length).toBe(0);
});

// A tab with neither a ticket nor a kept token asks nothing about a session.
it('asks nothing where there is nothing to ask with', async () => {
  const asking = host();
  expect(await establish(null, shelf(), asking)).toEqual({ kind: 'none', said: null });
  expect(asking.launch).not.toHaveBeenCalled();
  expect(asking.checkSession).not.toHaveBeenCalled();
});

// Storage that throws leaves the token in memory, for the life of the tab.
it('keeps the token in memory where storage throws', async () => {
  const refusing = {
    get length(): number {
      throw new DOMException('blocked', 'SecurityError');
    },
    key: () => {
      throw new DOMException('blocked', 'SecurityError');
    },
    getItem: () => {
      throw new DOMException('blocked', 'SecurityError');
    },
    setItem: () => {
      throw new DOMException('blocked', 'SecurityError');
    },
    removeItem: () => {
      throw new DOMException('blocked', 'SecurityError');
    },
    clear: () => undefined,
  } as Storage;
  const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  try {
    const shelved = shelf(() => refusing);
    expect(await establish(TICKET, shelved, host())).toEqual({ kind: 'held', expiresIn: 43_200 });
    expect(heldToken()).toBe(TOKEN);
    expect(shelved.kept()).toEqual([{ instanceId: 'host.1', token: TOKEN }]);

    const unavailable = shelf(() => {
      throw new DOMException('no storage here', 'SecurityError');
    });
    unavailable.keep('host.2', TOKEN);
    expect(unavailable.kept()).toEqual([{ instanceId: 'host.2', token: TOKEN }]);
  } finally {
    warn.mockRestore();
  }
});

// A refused exchange holds nothing, and says why.
it('holds nothing where the exchange is refused', async () => {
  const asking = host();
  asking.launch.mockRejectedValueOnce(
    new Refusal('rate_limited', 429, 'The explorer was opened too many times at once.'),
  );
  expect(await establish(TICKET, shelf(), asking)).toEqual({
    kind: 'none',
    said: 'The explorer was opened too many times at once.',
  });
  expect(heldToken()).toBeNull();
});

// A ticket refused as no session — spent already, or never issued — is the one
// thing the page says on its own, so the host's sentence for it is not carried.
it('carries no sentence where the exchange is refused as no session', async () => {
  const asking = host();
  asking.launch.mockRejectedValueOnce(
    new Refusal('authentication_required', 401, 'This explorer holds no session.'),
  );
  expect(await establish(TICKET, shelf(), asking)).toEqual({ kind: 'none', said: null });
  expect(heldToken()).toBeNull();
});
