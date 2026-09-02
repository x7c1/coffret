import { expect, it } from 'vitest';

import { isRefusal, refusalOf } from './refusal';

/** One answer of the server's refusal shape. */
function refused(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

it('reads a declined fetch as the reason it was declined', async () => {
  const refusal = await refusalOf(
    refused(409, {
      error: 'declined',
      message: 'no folder on this device holds this part of the Library',
      reason: 'unmapped',
    }),
  );

  expect(refusal.kind).toBe('declined');
  expect(refusal.status).toBe(409);
  expect(refusal.reason).toBe('unmapped');
  expect(refusal.surfaced).toBeNull();
  expect(refusal.message).toBe('no folder on this device holds this part of the Library');
  expect(isRefusal(refusal)).toBe(true);
});

// The finding is what tells one declined path from another, and it travels
// beside the reason rather than instead of it.
it('reads the finding a surfaced refusal stands on', async () => {
  const refusal = await refusalOf(
    refused(409, {
      error: 'declined',
      message: 'a file this device did not put there stands where this Entry belongs',
      reason: 'surfaced',
      surfaced: 'ForeignFile',
    }),
  );

  expect(refusal.reason).toBe('surfaced');
  expect(refusal.surfaced).toBe('ForeignFile');
});

it('reads a refusal that carries no reason', async () => {
  const refusal = await refusalOf(
    refused(404, { error: 'no_such_entry', message: 'the Library holds nothing at that path' }),
  );

  expect(refusal.kind).toBe('no_such_entry');
  expect(refusal.reason).toBeNull();
});

// The one refusal made before a route is reached. It says nothing about the
// Library and is a kind of its own for that reason: the screen it reaches has
// nothing to retry and nothing to show about a path.
it('reads a request the server would not answer at all', async () => {
  const refusal = await refusalOf(
    refused(403, {
      error: 'unauthorized',
      message: 'this Library is served only to whoever can read this device’s own files',
    }),
  );

  expect(refusal.kind).toBe('unauthorized');
  expect(refusal.status).toBe(403);
  expect(refusal.reason).toBeNull();
});

// A proxy's own error page stands where the server would have been. That is an
// ordinary thing to receive, and a parser that threw here would replace a
// refusal the screen can show with one it cannot.
it('does not throw on an answer that is not JSON', async () => {
  const refusal = await refusalOf(
    new Response('<html>502 Bad Gateway</html>', {
      status: 500,
      headers: { 'content-type': 'text/html' },
    }),
  );

  expect(refusal.kind).toBe('unrecognized');
  expect(refusal.status).toBe(500);
  expect(refusal.message).not.toBe('');
});

it('does not throw on JSON that is not a refusal', async () => {
  for (const body of [{ oops: true }, ['not an object'], null, 42]) {
    const refusal = await refusalOf(refused(500, body));
    expect(refusal.kind).toBe('unrecognized');
  }
});

// A server that grew a kind is not one this client can branch on, and saying so
// is better than passing the new name on as though it were one of the eight.
it('names a kind it has never heard of rather than passing it on', async () => {
  const refusal = await refusalOf(
    refused(418, { error: 'something_new', message: 'a kind from a later server' }),
  );

  expect(refusal.kind).toBe('unrecognized');
  expect(refusal.message).toBe('a kind from a later server');
});

it('drops a reason and a finding it has never heard of', async () => {
  const refusal = await refusalOf(
    refused(409, {
      error: 'declined',
      message: 'declined for a reason from a later server',
      reason: 'brand_new',
      surfaced: 'BrandNew',
    }),
  );

  expect(refusal.kind).toBe('declined');
  expect(refusal.reason).toBeNull();
  expect(refusal.surfaced).toBeNull();
});
