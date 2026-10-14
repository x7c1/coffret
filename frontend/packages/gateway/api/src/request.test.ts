import { afterEach, expect, it, vi } from 'vitest';

import { ORIGIN, TOKEN } from './page.testing';

import { dropToken, holdToken, onSessionEnding, withdrawAnswers } from './credential';
import { apiUrl, asked, askedForJson, ForeignUrl, sentForJson } from './request';
import { isRefusal } from './refusal';

afterEach(() => {
  vi.unstubAllGlobals();
});

/** Answers every request with one response. */
function answering(response: Response | Error) {
  vi.stubGlobal('fetch', () =>
    response instanceof Error ? Promise.reject(response) : Promise.resolve(response),
  );
}

it('builds a route URL, with a query only where it was given one', () => {
  expect(apiUrl('list')).toBe('/api/list');
  expect(apiUrl('list', { path: 'albums/2026' })).toBe('/api/list?path=albums%2F2026');
});

// Stopping the server has to leave a sentence on the screen rather than a
// silent hang, so a request that never got an answer is a refusal like any
// other.
it('turns a request that got no answer into a refusal', async () => {
  answering(new TypeError('fetch failed'));

  const thrown = await askedForJson('/api/library').catch((refusal: unknown) => refusal);
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('unreachable');
    expect(thrown.status).toBe(0);
  }
});

it('answers with the body a route served', async () => {
  answering(Response.json({ name: 'main', library_id: '11', provider: 's3' }));

  await expect(askedForJson(apiUrl('library'))).resolves.toEqual({
    name: 'main',
    library_id: '11',
    provider: 's3',
  });
});

it('refuses an answer that is not JSON at all', async () => {
  answering(new Response('<html>a proxy</html>', { status: 200 }));

  const thrown = await askedForJson('/api/library').catch((refusal: unknown) => refusal);
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('unrecognized');
  }
});

// A request that was sending a body when it failed may well have been answered:
// the server refuses a drop it will not take while the browser is still sending
// it (spec: LA-10). So it is not said to be a server that did not answer.
it('does not say a request that broke while sending got no answer', async () => {
  answering(new TypeError('fetch failed'));

  const thrown = await askedForJson('/api/upload', undefined, 'POST', 'a body').catch(
    (refusal: unknown) => refusal,
  );
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('unreachable');
    expect(thrown.message).not.toContain('did not answer');
    expect(thrown.message).toContain('broke off while it was being sent');
  }
});

/** Every request `fetch` was asked to make, and what it was asked with. */
function recording(response: () => Response): { url: string; init: RequestInit }[] {
  const made: { url: string; init: RequestInit }[] = [];
  vi.stubGlobal('fetch', (url: string, init: RequestInit) => {
    made.push({ url, init });
    return Promise.resolve(response());
  });
  return made;
}

/** A refusal of the explorer host's, as it sends one. */
function hostRefusal(status: number, error: string): Response {
  return Response.json({ error, message: `the host's sentence for ${error}` }, { status });
}

// LA-6, LA-14: every request carries the browser token in `Authorization`, and
// nothing else of the browser's: no cookie, no redirect followed, nothing
// kept in the browser's cache.
it('carries the browser token and nothing else of the browser', async () => {
  const made = recording(() => Response.json({}));

  await askedForJson(apiUrl('library'));

  expect(made).toHaveLength(1);
  const { init } = made[0];
  expect(init.headers).toEqual({ Authorization: `Bearer ${TOKEN}` });
  expect(init.credentials).toBe('omit');
  expect(init.redirect).toBe('error');
  expect(init.cache).toBe('no-store');
});

// LA-6: the token goes to the page's own host and nowhere else, and only under
// the two prefixes the host answers.
it('refuses to ask anywhere but its own host', async () => {
  const made = recording(() => Response.json({}));

  for (const url of [
    'https://elsewhere.example/api/library',
    '//elsewhere.example/api/library',
    'http://127.0.0.1:5174/api/library',
    'http://someone@127.0.0.1:5173/api/library',
    '/assets/index.js',
    '/api',
    '/api/../elsewhere',
    '/authority',
  ]) {
    await expect(asked(url), url).rejects.toBeInstanceOf(ForeignUrl);
    await expect(sentForJson(url, 'a body'), url).rejects.toBeInstanceOf(ForeignUrl);
  }
  expect(made).toHaveLength(0);

  await asked(`${ORIGIN}/api/library`);
  await asked('/auth/session');
  expect(made).toHaveLength(2);
});

// A page that holds no token asks nothing at all.
it('asks nothing without a token', async () => {
  const made = recording(() => Response.json({}));
  dropToken();

  const thrown = await asked(apiUrl('library')).catch((refusal: unknown) => refusal);
  expect(isRefusal(thrown) && thrown.kind).toBe('authentication_required');
  expect(made).toHaveLength(0);
});

// LA-14, LA-15: a refusal saying the session is gone, or the Library locked, is
// told to whoever listens — and a refusal of the server's own is not.
it('says when an answer ended the session or found the Library locked', async () => {
  const heard: string[] = [];
  const stop = onSessionEnding((ending, refusal) => heard.push(`${ending}:${refusal.kind}`));
  try {
    for (const [status, error] of [
      [401, 'authentication_required'],
      [502, 'server_unavailable'],
      [423, 'library_locked'],
      [423, 'locked'],
      [502, 'bad_gateway'],
      [409, 'conflict'],
    ] as const) {
      recording(() => hostRefusal(status, error));
      await asked(apiUrl('list')).catch(() => undefined);
    }
  } finally {
    stop();
  }
  expect(heard).toEqual([
    'ended:authentication_required',
    'ended:server_unavailable',
    'locked:library_locked',
  ]);
});

// A mutation refused because the session ended or the Library locked stays
// refused: nothing here sends it again.
it('never sends a refused mutation again', async () => {
  for (const [status, error] of [
    [401, 'authentication_required'],
    [423, 'library_locked'],
  ] as const) {
    const made = recording(() => hostRefusal(status, error));
    const thrown = await asked(apiUrl('delete'), undefined, 'POST').catch((r: unknown) => r);
    expect(isRefusal(thrown) && thrown.kind).toBe(error);
    await Promise.resolve();
    expect(made).toHaveLength(1);
  }
});

// An answer that arrives after the session it was asked under ended, or after
// the Library locked, is let go of rather than handed to whoever asked.
it('lets go of an answer that arrives after the session ended or the Library locked', async () => {
  for (const end of [dropToken, withdrawAnswers]) {
    let answer: (response: Response) => void = () => undefined;
    vi.stubGlobal(
      'fetch',
      () => new Promise<Response>((resolve) => (answer = resolve)),
    );
    const asking = askedForJson(apiUrl('list')).catch((refusal: unknown) => refusal);
    end();
    answer(Response.json({ path: '', folders: [], files: [] }));
    const thrown = await asking;
    expect(isRefusal(thrown) && thrown.kind).toBe('withdrawn');
    holdToken(TOKEN);
  }
});
