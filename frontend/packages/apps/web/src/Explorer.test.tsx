import { act, cleanup, render, screen } from '@testing-library/react';
import { useEffect, useState } from 'react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import {
  dropToken,
  getFile,
  getLibrary,
  heldToken,
  holdToken,
  isRefusal,
  Refusal,
} from '@coffret/api';

import { Explorer, HOW_TO_OPEN, LOCKED_POLL_MS, NO_SESSION } from './Explorer';
import type { Page } from './pages';
import { ReaderView } from './ReaderView';
import { establish, shelf, type Opened } from './session';

// The pages the reader fetches, as it reaches them. Everything else the package
// exports is the real thing — the token it holds and the requests it makes
// above all, which go to the `fetch` each case stands in for.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getFile: vi.fn(),
}));

const fetched = vi.mocked(getFile);
const TOKEN = 'k'.repeat(43);

/** Every request the page made, by path, and how each was answered. */
let made: string[] = [];
/** The Library's state as the host's session route says it. */
let library: 'locked' | 'unlocked' = 'unlocked';
/** What everything under `/api` is answered with. */
let api: () => Response = () => Response.json({ name: 'main', library_id: '1', provider: 's3' });

/** Object URLs made, and the ones revoked. */
let created: string[] = [];
let revoked: string[] = [];

beforeEach(() => {
  made = [];
  library = 'unlocked';
  api = () => Response.json({ name: 'main', library_id: '1', provider: 's3' });
  created = [];
  revoked = [];
  vi.stubGlobal('fetch', (url: string) => {
    const path = new URL(url, window.location.origin).pathname;
    made.push(path);
    if (path === '/auth/session') {
      return Promise.resolve(Response.json({ expires_in: 600, library }));
    }
    if (path === '/auth/session/revoke') {
      return Promise.resolve(new Response(null, { status: 204 }));
    }
    return Promise.resolve(api());
  });
  URL.createObjectURL = () => {
    const url = `blob:page-${created.length + 1}`;
    created.push(url);
    return url;
  };
  URL.revokeObjectURL = (url: string) => {
    revoked.push(url);
  };
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  dropToken();
});

/** A tab that came up holding a token. */
function held(expiresIn = 43_200): Promise<Opened> {
  holdToken(TOKEN);
  return Promise.resolve({ kind: 'held', expiresIn });
}

/** A screen that asks one question of the server as it is drawn, and shows the answer. */
function Asking() {
  const [said, setSaid] = useState('asking');
  useEffect(() => {
    void getLibrary().then(
      (answer) => setSaid(`library ${answer.name}`),
      (refused: unknown) => setSaid(isRefusal(refused) ? `refused ${refused.kind}` : 'failed'),
    );
  }, []);
  return <p>{said}</p>;
}

/** Answers to `/api` that a case releases by hand. */
function heldBack(): { release: (response: Response) => void } {
  const waiting: ((response: Response) => void)[] = [];
  vi.stubGlobal('fetch', (url: string) => {
    made.push(new URL(url, window.location.origin).pathname);
    return new Promise<Response>((resolve) => waiting.push(resolve));
  });
  return {
    release: (response) => {
      for (const resolve of waiting.splice(0)) {
        resolve(response.clone());
      }
    },
  };
}

// LA-21: a tab holding no token says how the explorer is opened, and reads no
// Library data at all.
it('shows only how to open the explorer without a token, and asks nothing', async () => {
  render(
    <Explorer
      opened={Promise.resolve({ kind: 'none', said: null })}
      shelved={shelf()}
      screen={Asking}
    />,
  );
  expect(await screen.findByText(NO_SESSION)).toBeTruthy();
  expect(screen.getByText(HOW_TO_OPEN)).toBeTruthy();
  expect(screen.queryByText('asking')).toBeNull();
  expect(made).toEqual([]);
});

// LA-14: a `401` is the session gone: the token is let go of and forgotten, and
// the screen goes.
it('lets the token go on a 401, and shows how to open the explorer', async () => {
  window.sessionStorage.setItem('coffret.explorer.session.host.1', TOKEN);
  api = () =>
    Response.json({ error: 'authentication_required', message: 'no session' }, { status: 401 });
  render(<Explorer opened={held()} shelved={shelf()} screen={Asking} />);

  expect(await screen.findByText(NO_SESSION)).toBeTruthy();
  expect(heldToken()).toBeNull();
  expect(window.sessionStorage.length).toBe(0);
  expect(made).toEqual(['/api/library']);
});

// LA-14, DK-4: a `423` from the host is the Library locked: the screen is
// drawn again in the locked state, and the host's session route — not the
// server's work route — is asked until it is unlocked. Nothing refused is sent
// again on its own.
it('shows the locked state on a 423 and asks the host, not the server, until unlocked', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  library = 'locked';
  api = () =>
    Response.json({ error: 'library_locked', message: 'the Library is locked' }, { status: 423 });
  render(<Explorer opened={held()} shelved={shelf()} screen={Asking} />);

  expect(await screen.findByText('refused library_locked')).toBeTruthy();
  expect(await screen.findByText(/waits for it to be unlocked/)).toBeTruthy();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(LOCKED_POLL_MS * 3);
  });
  const asked = made.filter((path) => path !== '/auth/session');
  // Once by the first screen, once by the screen drawn locked, and no more:
  // the polling is the host's session route alone.
  expect(asked).toEqual(['/api/library', '/api/library']);
  expect(made.filter((path) => path === '/auth/session').length).toBeGreaterThanOrEqual(3);
  expect(made).not.toContain('/api/work');
  expect(heldToken()).toBe(TOKEN);

  // Unlocked: the screen is drawn afresh, and asks again.
  library = 'unlocked';
  api = () => Response.json({ name: 'main', library_id: '1', provider: 's3' });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(LOCKED_POLL_MS);
  });
  expect(await screen.findByText('library main')).toBeTruthy();
  expect(screen.queryByText(/waits for it to be unlocked/)).toBeNull();
});

// A launch file whose ticket was spent already is answered by the host as a
// request with no session, in its own words. The page says that itself, with
// how to open the explorer, and says it once.
it('says once that the tab holds no session where the ticket was refused as such', async () => {
  const HOST_SAYS = 'This explorer holds no session. Open the explorer from the coffret app.';
  const opened = establish('t'.repeat(43), shelf(), {
    launch: () => Promise.reject(new Refusal('authentication_required', 401, HOST_SAYS)),
    checkSession: () => Promise.reject(new Error('not asked')),
  });
  render(<Explorer opened={opened} shelved={shelf()} screen={Asking} />);

  expect(await screen.findByText(NO_SESSION)).toBeTruthy();
  expect(screen.queryByText(HOST_SAYS)).toBeNull();
  expect(screen.getAllByText(/holds no session/)).toHaveLength(1);
});

// The page offers no control that ends its own session: that is the desktop
// app's, from its tray, for every tab at once (spec: LA-18).
it('offers no control to disconnect the tab', async () => {
  render(<Explorer opened={held()} shelved={shelf()} screen={Asking} />);
  await screen.findByText('library main');
  expect(screen.queryByRole('button')).toBeNull();
  expect(screen.queryByText(/disconnect/i)).toBeNull();
});

// An answer that arrives after the Library locked under it, or after the
// session ended, is not drawn.
it('draws no answer that arrives after the lock or the end of the session', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const gate = heldBack();
  render(<Explorer opened={held(60)} shelved={shelf()} screen={Asking} />);
  await screen.findByText('asking');
  await act(async () => {
    await Promise.resolve();
  });

  // The session runs out while the question is in flight; then its answer
  // lands.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  await act(async () => {
    gate.release(Response.json({ name: 'main', library_id: '1', provider: 's3' }));
  });
  expect(await screen.findByText(NO_SESSION)).toBeTruthy();
  expect(screen.queryByText('library main')).toBeNull();
});

/** A screen that is a reader open on two pages. */
const PAGES: Page[] = [
  { path: 'albums/one.jpg', name: 'one.jpg', remote: false },
  { path: 'albums/two.jpg', name: 'two.jpg', remote: false },
];

function Reading() {
  return (
    <ReaderView
      pages={PAGES}
      at={0}
      discarded={0}
      onNavigate={() => undefined}
      onClose={() => undefined}
      onFetching={() => undefined}
      onFetched={() => undefined}
      reconnect={null}
    />
  );
}

// What a lock or a revocation drops includes every page the reader held:
// their object URLs are revoked, and a page whose bytes land afterwards is
// revoked as it lands rather than drawn.
it('revokes every object URL the reader held when the Library locks, and draws none that lands later', async () => {
  let late: (blob: Blob) => void = () => undefined;
  fetched.mockImplementation((path: string) =>
    path === 'albums/one.jpg'
      ? Promise.resolve(new Blob(['one']))
      : new Promise<Blob>((resolve) => (late = resolve)),
  );
  render(<Explorer opened={held()} shelved={shelf()} screen={Reading} />);
  expect(await screen.findByAltText('one.jpg')).toBeTruthy();
  expect(created).toEqual(['blob:page-1']);

  // The Library locks, as an answer to anything under `/api` says.
  fetched.mockImplementation(() => new Promise<Blob>(() => undefined));
  library = 'locked';
  api = () =>
    Response.json({ error: 'library_locked', message: 'the Library is locked' }, { status: 423 });
  await act(async () => {
    await getLibrary().catch(() => undefined);
  });
  expect(revoked).toContain('blob:page-1');

  // The page being prefetched when it locked lands now, and is nobody's.
  await act(async () => {
    late(new Blob(['two']));
  });
  expect(created).toEqual(['blob:page-1', 'blob:page-2']);
  expect(revoked).toContain('blob:page-2');
  expect(screen.queryByAltText('one.jpg')).toBeNull();
  expect(screen.queryByAltText('two.jpg')).toBeNull();
});

it('revokes every object URL the reader held when the session ends', async () => {
  fetched.mockResolvedValue(new Blob(['page']));
  render(<Explorer opened={held()} shelved={shelf()} screen={Reading} />);
  expect(await screen.findByAltText('one.jpg')).toBeTruthy();

  api = () =>
    Response.json({ error: 'authentication_required', message: 'no session' }, { status: 401 });
  await act(async () => {
    await getLibrary().catch(() => undefined);
  });
  expect(await screen.findByText(NO_SESSION)).toBeTruthy();
  expect(revoked).toEqual(expect.arrayContaining(created));
  expect(created.length).toBeGreaterThan(0);
});

// LA-18: nothing renews a session, and the screen goes when it runs out.
it('lets the session go when it runs out', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  render(<Explorer opened={held(60)} shelved={shelf()} screen={Asking} />);
  await screen.findByText('library main');

  await act(async () => {
    await vi.advanceTimersByTimeAsync(60_000);
  });
  expect(screen.getByText(NO_SESSION)).toBeTruthy();
  expect(heldToken()).toBeNull();
});
