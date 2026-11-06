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
  type ListedFile,
  type Listing,
} from '@coffret/api';

import { ENDED, Explorer, HOW_TO_OPEN, LOCKED_POLL_MS, NO_SESSION } from './Explorer';
import { LOCKED_HEADING } from './Locked';
import { ReaderView } from './ReaderView';
import { establish, KEPT_PREFIX, shelf, type DesktopSession, type Opened } from './session';
import { fileKey, type FileRow } from './tree';

// The pages the reader fetches, as it reaches them. Everything else the package
// exports is the real thing — the token it holds and the requests it makes
// above all, which go to the `fetch` each case stands in for.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getFile: vi.fn(),
}));

const fetched = vi.mocked(getFile);
const TOKEN = 'k'.repeat(43);
/** The token a renewal brings. */
const RENEWED = 'n'.repeat(43);
/** A fresh ticket the desktop app hands over. */
const FRESH = 'f'.repeat(43);

/** Every request the page made, by path, and how each was answered. */
let made: string[] = [];
/** Every request the page made, by path, with the `Authorization` it carried. */
let carried: { path: string; authorization: string | undefined }[] = [];
/** What an exchange of a launch ticket is answered with. */
let launched: () => Response = () =>
  Response.json({ token: RENEWED, expires_in: 43_200, instance_id: 'host.1' });
/** The Library's state as the host's session route says it. */
let library: 'locked' | 'unlocked' = 'unlocked';
/** What everything under `/api` is answered with. */
let api: () => Response = () => Response.json({ name: 'main', library_id: '1', provider: 's3' });

/** Object URLs made, and the ones revoked. */
let created: string[] = [];
let revoked: string[] = [];

beforeEach(() => {
  made = [];
  carried = [];
  launched = () => Response.json({ token: RENEWED, expires_in: 43_200, instance_id: 'host.1' });
  library = 'unlocked';
  api = () => Response.json({ name: 'main', library_id: '1', provider: 's3' });
  created = [];
  revoked = [];
  vi.stubGlobal('fetch', (url: string, init?: RequestInit) => {
    const path = new URL(url, window.location.origin).pathname;
    made.push(path);
    carried.push({ path, authorization: (init?.headers as Record<string, string> | undefined)?.Authorization });
    if (path === '/auth/session') {
      return Promise.resolve(Response.json({ expires_in: 600, library }));
    }
    if (path === '/auth/session/revoke') {
      return Promise.resolve(new Response(null, { status: 204 }));
    }
    if (path === '/auth/launch') {
      return Promise.resolve(launched());
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

/** A page that came up holding a token. */
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

// LA-21: a page holding no token says how the explorer is opened, and reads no
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
});

// A launch file whose ticket was spent already is answered by the host as a
// request with no session, in its own words. The page says that itself, with
// how to open the explorer, and says it once.
it('says once that the page holds no session where the ticket was refused as such', async () => {
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
// app's, by quitting, for every session at once (spec: LA-18).
it('offers no control to disconnect the explorer', async () => {
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

function page(name: string): ListedFile {
  return {
    name,
    path: `albums/${name}`,
    size: 4,
    mtime: null,
    deposited_at: null,
    state: 'present',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
  };
}

const ALBUM: Listing = {
  path: 'albums',
  mapped: true,
  held: true,
  folders: [],
  files: [page('one.jpg'), page('two.jpg')],
  folders_on_disk: [],
  empty_mapped_prefixes: [],
};

const FIRST: FileRow = {
  kind: 'file',
  key: fileKey('albums/one.jpg'),
  path: 'albums/one.jpg',
  name: 'one.jpg',
  depth: 1,
  file: ALBUM.files[0],
  in: ALBUM,
};

/** A screen that is the reader's sheet open on the first of two pages. */
function Reading() {
  return (
    <ReaderView
      row={FIRST}
      asked={undefined}
      counter={{ at: 1, of: 2 }}
      discarded={0}
      escapable
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

// LA-18: a session is never extended, and where no new token was taken up the
// screen goes when it runs out.
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

// DK-1, LA-14: a Library that locks while somebody is looking brings the
// app's Passphrase window forward by itself. Behind the host every request
// under `/api` but the unlock is refused as locked — the work answer too — so
// the explorer's own screen hears the lock only as those refusals.
it('asks for the unlock on its own once the host refuses the screen as locked', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const focused = vi.spyOn(document, 'hasFocus').mockReturnValue(true);
  library = 'locked';
  vi.stubGlobal('fetch', (url: string, init?: RequestInit) => {
    const path = new URL(url, window.location.origin).pathname;
    made.push(`${init?.method ?? 'GET'} ${path}`);
    if (path === '/auth/session') {
      return Promise.resolve(Response.json({ expires_in: 600, library }));
    }
    if (path === '/api/unlock') {
      return Promise.resolve(
        Response.json({ library: 'locked', message: 'asking in the app' }, { status: 202 }),
      );
    }
    return Promise.resolve(
      Response.json({ error: 'library_locked', message: 'the Library is locked' }, { status: 423 }),
    );
  });
  render(<Explorer opened={held()} shelved={shelf()} />);

  // The screen's own locked state, said once, and nothing about a tab.
  expect(await screen.findByRole('heading', { name: LOCKED_HEADING })).toBeTruthy();
  expect(document.body.textContent).not.toMatch(/\btab\b/i);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(LOCKED_POLL_MS * 3);
  });
  expect(made.filter((asked) => asked === 'POST /api/unlock')).toHaveLength(1);
  focused.mockRestore();
});

/** The desktop app's window, as the page sees it: tickets it hands over, and what it is told. */
function desktopWindow() {
  const listeners = new Set<(ticket: string) => void>();
  const sessionEnded = vi.fn();
  const session: DesktopSession = {
    onTicket(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    sessionEnded,
  };
  return {
    session,
    sessionEnded,
    listening: () => listeners.size,
    /** Hands `ticket` over, as the app does ahead of the token's expiry. */
    async deliver(ticket: string) {
      await act(async () => {
        for (const listener of [...listeners]) {
          listener(ticket);
        }
        await new Promise((resolve) => setTimeout(resolve, 0));
      });
    },
  };
}

/** A screen that counts how often it was drawn afresh, and asks the server on each press of its button. */
let mounts = 0;
function Counting() {
  const [said, setSaid] = useState('asking');
  useEffect(() => {
    mounts += 1;
    void getLibrary().then(
      (answer) => setSaid(`library ${answer.name}`),
      (refused: unknown) => setSaid(isRefusal(refused) ? `refused ${refused.kind}` : 'failed'),
    );
  }, []);
  const ask = () => {
    void getLibrary().then(
      (answer) => setSaid(`again ${answer.name}`),
      (refused: unknown) => setSaid(isRefusal(refused) ? `refused ${refused.kind}` : 'failed'),
    );
  };
  return (
    <>
      <p>{said}</p>
      <input aria-label="kept" defaultValue="" />
      <span onClick={ask}>ask again</span>
    </>
  );
}

// LA-18: in the desktop app's window, a fresh ticket handed over ahead of the
// token's expiry is exchanged for a new token, held for every request from
// then on and kept in place of the old one, whose session is revoked with its
// own token. The screen is not drawn again: what it shows stays.
it('takes a delivered ticket up as a new token in place, without drawing the screen again', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  mounts = 0;
  const desktop = desktopWindow();
  const shelved = shelf();
  shelved.keep('host.1', TOKEN);
  render(<Explorer opened={held(3_600)} shelved={shelved} screen={Counting} desktop={desktop.session} />);
  await screen.findByText('library main');
  const kept = screen.getByLabelText<HTMLInputElement>('kept');
  kept.value = 'typed before';
  expect(desktop.listening()).toBe(1);

  await desktop.deliver(FRESH);

  expect(heldToken()).toBe(RENEWED);
  expect(window.sessionStorage.getItem(`${KEPT_PREFIX}host.1`)).toBe(RENEWED);
  expect(window.sessionStorage.length).toBe(1);
  const launch = carried.find((sent) => sent.path === '/auth/launch');
  expect(launch?.authorization).toBe(`Launch ${FRESH}`);
  const revoke = carried.find((sent) => sent.path === '/auth/session/revoke');
  expect(revoke?.authorization).toBe(`Bearer ${TOKEN}`);

  // Every later request carries the new token, from the same screen.
  await act(async () => {
    screen.getByText('ask again').click();
    await Promise.resolve();
  });
  expect(await screen.findByText('again main')).toBeTruthy();
  expect(carried.filter((sent) => sent.path === '/api/library').map((sent) => sent.authorization)).toEqual([
    `Bearer ${TOKEN}`,
    `Bearer ${RENEWED}`,
  ]);
  expect(mounts).toBe(1);
  expect(screen.getByLabelText<HTMLInputElement>('kept').value).toBe('typed before');
  expect(desktop.sessionEnded).not.toHaveBeenCalled();

  // The old session's clock no longer counts: the new one's does.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(3_600_000);
  });
  expect(screen.getByText('again main')).toBeTruthy();
  expect(heldToken()).toBe(RENEWED);
});

// LA-18: a locked Library keeps its sessions, so a ticket handed over while it
// is locked is taken up all the same, and the page goes on showing the locked
// state, asking the host with the new token.
it('takes a delivered ticket up while the Library is locked, and stays locked', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  mounts = 0;
  const desktop = desktopWindow();
  const shelved = shelf();
  shelved.keep('host.1', TOKEN);
  library = 'locked';
  api = () =>
    Response.json({ error: 'library_locked', message: 'the Library is locked' }, { status: 423 });
  render(<Explorer opened={held()} shelved={shelved} screen={Counting} desktop={desktop.session} />);
  expect(await screen.findByText('refused library_locked')).toBeTruthy();
  const drawn = mounts;

  await desktop.deliver(FRESH);
  expect(heldToken()).toBe(RENEWED);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(LOCKED_POLL_MS * 2);
  });
  expect(screen.getByText('refused library_locked')).toBeTruthy();
  expect(mounts).toBe(drawn);
  const asked = carried.filter((sent) => sent.path === '/auth/session');
  expect(asked[asked.length - 1].authorization).toBe(`Bearer ${RENEWED}`);
  expect(desktop.sessionEnded).not.toHaveBeenCalled();
});

// LA-18: the locked page asks the host with the token it holds; an answer to
// the token before a renewal, refused because the renewal revoked it, is not
// the session ending.
it('keeps the session where a locked poll with the token renewed away is refused', async () => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  const desktop = desktopWindow();
  const shelved = shelf();
  shelved.keep('host.1', TOKEN);
  library = 'locked';
  api = () =>
    Response.json({ error: 'library_locked', message: 'the Library is locked' }, { status: 423 });
  let release: (() => void) | null = null;
  const real = globalThis.fetch;
  vi.stubGlobal('fetch', (url: string, init?: RequestInit) => {
    const path = new URL(url, window.location.origin).pathname;
    const authorization = (init?.headers as Record<string, string> | undefined)?.Authorization;
    if (path === '/auth/session' && authorization === `Bearer ${TOKEN}` && heldToken() === TOKEN && release === null) {
      // The poll with the old token is answered only once the renewal has
      // revoked it.
      return new Promise<Response>((resolve) => {
        release = () =>
          resolve(Response.json({ error: 'authentication_required', message: 'no session' }, { status: 401 }));
      });
    }
    return real(url, init);
  });
  render(<Explorer opened={held()} shelved={shelved} screen={Counting} desktop={desktop.session} />);
  expect(await screen.findByText('refused library_locked')).toBeTruthy();
  await vi.waitFor(() => expect(release).not.toBeNull());

  await desktop.deliver(FRESH);
  expect(heldToken()).toBe(RENEWED);
  await act(async () => {
    release?.();
    await vi.advanceTimersByTimeAsync(0);
  });

  expect(screen.getByText('refused library_locked')).toBeTruthy();
  expect(heldToken()).toBe(RENEWED);
  expect(desktop.sessionEnded).not.toHaveBeenCalled();
});

// LA-17, LA-18: an exchange of a delivered ticket that is refused — the ticket
// spent or expired, or exchanges past the host's limit — ends the session,
// and the page tells the desktop app, which loads its window afresh.
it.each([
  [401, 'authentication_required', NO_SESSION],
  [429, 'rate_limited', 'too many exchanges'],
] as const)('reports a refused exchange of a delivered ticket (%i)', async (status, error, shown) => {
  const desktop = desktopWindow();
  launched = () => Response.json({ error, message: 'too many exchanges' }, { status });
  render(<Explorer opened={held()} shelved={shelf()} screen={Counting} desktop={desktop.session} />);
  await screen.findByText('library main');

  await desktop.deliver(FRESH);

  expect(await screen.findByText(shown)).toBeTruthy();
  expect(heldToken()).toBeNull();
  expect(desktop.sessionEnded).toHaveBeenCalled();
  expect(desktop.listening()).toBe(0);
});

// LA-21: a page in the desktop app's window whose session ended under it — the
// host no longer admits its token, or the server behind it stopped — tells the
// app, and so does one that came up holding none.
it('tells the desktop app when its session ends, or never came', async () => {
  const desktop = desktopWindow();
  render(<Explorer opened={held()} shelved={shelf()} screen={Counting} desktop={desktop.session} />);
  await screen.findByText('library main');
  expect(desktop.sessionEnded).not.toHaveBeenCalled();

  api = () =>
    Response.json({ error: 'authentication_required', message: 'no session' }, { status: 401 });
  await act(async () => {
    await getLibrary().catch(() => undefined);
  });
  expect(await screen.findByText(ENDED)).toBeTruthy();
  expect(desktop.sessionEnded).toHaveBeenCalledTimes(1);
  cleanup();

  const never = desktopWindow();
  render(
    <Explorer
      opened={Promise.resolve({ kind: 'none', said: null })}
      shelved={shelf()}
      screen={Counting}
      desktop={never.session}
    />,
  );
  expect(await screen.findByText(NO_SESSION)).toBeTruthy();
  expect(never.sessionEnded).toHaveBeenCalledTimes(1);
});

// No string the page shows calls the explorer a tab: it is a window in the
// desktop app, and a page in a browser.
it('never calls the explorer a tab', () => {
  for (const said of [NO_SESSION, HOW_TO_OPEN, ENDED]) {
    expect(said).not.toMatch(/\btab\b/i);
  }
});
