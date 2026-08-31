import { afterEach, expect, it, vi } from 'vitest';

import { apiUrl, askedForJson } from './request';
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
