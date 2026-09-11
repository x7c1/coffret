import { expect, it } from 'vitest';

import type { DeclinedReason, SurfacedFinding } from './refusal';
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

// The other half of the round trip. The backend fixes these literals in its own
// case over every finding it can build; this fixes the same six here, so a
// rename or a typo on either side is caught rather than falling through to
// `null` on one of them and being read as "no finding" by every screen.
it('reads every finding name the server can send', async () => {
  const names: SurfacedFinding[] = [
    'ForeignFile',
    'LocallyChanged',
    'WitnessedDeletion',
    'UnreachablePlace',
    'KeyLost',
    'ReservedComponent',
  ];

  for (const name of names) {
    const refusal = await refusalOf(
      refused(409, {
        error: 'declined',
        message: 'the fetch found something about this Entry',
        reason: name === 'KeyLost' ? 'locked' : 'surfaced',
        surfaced: name,
      }),
    );

    expect(refusal.surfaced, `${name} is a finding this client knows`).toBe(name);
  }
});

// The reason is fixed on both sides the way the finding is, and for the same
// stake: one the decoder has not heard of is dropped to `null`, which a screen
// reads as a refusal with no reason rather than as a client that is behind. Each
// literal here is one the server builds — `reserved` for a path carrying a name
// coffret keeps for itself, `refused_root` for a mapped folder that is not the
// one its mapping was recorded against — and none of them is asserted anywhere
// else on this side.
it('reads every declined reason the server can send', async () => {
  const reasons: DeclinedReason[] = [
    'unmapped',
    'unmaterializable',
    'reserved',
    'refused_root',
    'surfaced',
    'locked',
    'pack_resident',
  ];

  for (const reason of reasons) {
    const refusal = await refusalOf(
      refused(409, { error: 'declined', message: 'the server declined this one', reason }),
    );

    expect(refusal.reason, `${reason} is a reason this client knows`).toBe(reason);
  }
});

// EP-13: a folder this device maps is not the folder it was set up against. Its
// own reason, because nothing on a page settles it and the sentence is the whole
// of what a screen shows.
it('reads a refused mapped root as its own declined reason', async () => {
  const refusal = await refusalOf(
    refused(409, {
      error: 'declined',
      message:
        'a folder this device maps is not the folder it was set up against, so nothing was ' +
        'put into it; record the mapping again with `coffret map`',
      reason: 'refused_root',
    }),
  );

  expect(refusal.kind).toBe('declined');
  expect(refusal.reason).toBe('refused_root');
  expect(refusal.surfaced).toBeNull();
  expect(refusal.message).toContain('coffret map');
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
      message: "this Library is served only to whoever can read this device's own files",
    }),
  );

  expect(refusal.kind).toBe('unauthorized');
  expect(refusal.status).toBe(403);
  expect(refusal.reason).toBeNull();
});

// The server was locked, by a person or by the interval it went unasked for.
// Its own kind and not `unauthorized`: that one is said to somebody who is not
// the owner of this Library and tells them nothing, and this is said to the
// owner about their own device — so the sentence is the whole answer, and it is
// the one thing a screen shows verbatim.
it('reads a server that has locked itself', async () => {
  const refusal = await refusalOf(
    refused(423, {
      error: 'locked',
      message:
        'the Passphrase is required: this server is locked, either because it was asked to ' +
        'be or because nothing had used it for a while, and it is unlocked by starting it ' +
        'again with the Passphrase',
    }),
  );

  expect(refusal.kind).toBe('locked');
  expect(refusal.status).toBe(423);
  expect(refusal.reason).toBeNull();
  expect(refusal.message).toContain('Passphrase');
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
// is better than passing the new name on as though it were one of the nine.
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
