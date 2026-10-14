import { afterEach, expect, it, vi } from 'vitest';

import { checkSession, launch, revokeSession } from './auth';
import { dropToken, onSessionEnding } from './credential';
import { isRefusal } from './refusal';

afterEach(() => {
  vi.unstubAllGlobals();
});

/** Every request `fetch` was asked to make, answered with `response`. */
function recording(response: () => Response): { url: string; init: RequestInit }[] {
  const made: { url: string; init: RequestInit }[] = [];
  vi.stubGlobal('fetch', (url: string, init: RequestInit) => {
    made.push({ url, init });
    return Promise.resolve(response());
  });
  return made;
}

const TICKET = 'a'.repeat(43);

// LA-16: a ticket is exchanged in `Authorization: Launch`, with the JSON body
// the route takes, before any token is held.
it('exchanges a launch ticket in the header the host reads it from', async () => {
  dropToken();
  const made = recording(() =>
    Response.json({ token: 'b'.repeat(43), expires_in: 43_200, instance_id: 'host.0' }),
  );

  await expect(launch(TICKET)).resolves.toEqual({
    token: 'b'.repeat(43),
    expires_in: 43_200,
    instance_id: 'host.0',
  });
  expect(made).toHaveLength(1);
  expect(made[0].url).toBe('/auth/launch');
  expect(made[0].init.method).toBe('POST');
  expect(made[0].init.headers).toEqual({
    Authorization: `Launch ${TICKET}`,
    'Content-Type': 'application/json',
  });
  expect(made[0].init.body).toBe('{}');
  expect(made[0].init.credentials).toBe('omit');
});

// A refused exchange is the page's to read, and ends no session it holds.
it('leaves a refused exchange to whoever asked', async () => {
  recording(() =>
    Response.json({ error: 'authentication_required', message: 'spent' }, { status: 401 }),
  );
  const heard = vi.fn();
  const stop = onSessionEnding(heard);
  try {
    const thrown = await launch(TICKET).catch((refusal: unknown) => refusal);
    expect(isRefusal(thrown) && thrown.kind).toBe('authentication_required');
    const checked = await checkSession('c'.repeat(43)).catch((refusal: unknown) => refusal);
    expect(isRefusal(checked) && checked.kind).toBe('authentication_required');
  } finally {
    stop();
  }
  expect(heard).not.toHaveBeenCalled();
});

// LA-16: a session is asked about with the token being asked about, which need
// not be the one held.
it('asks about a session with the token being asked about', async () => {
  const made = recording(() => Response.json({ expires_in: 60, library: 'locked' }));

  await expect(checkSession('c'.repeat(43))).resolves.toEqual({
    expires_in: 60,
    library: 'locked',
  });
  expect(made[0].url).toBe('/auth/session');
  expect(made[0].init.headers).toEqual({ Authorization: `Bearer ${'c'.repeat(43)}` });
});

it('ends the session of the token it is given', async () => {
  const made = recording(() => new Response(null, { status: 204 }));
  await revokeSession('r'.repeat(43));
  expect(made[0].url).toBe('/auth/session/revoke');
  expect(made[0].init.method).toBe('POST');
  expect(made[0].init.headers).toEqual({ Authorization: `Bearer ${'r'.repeat(43)}` });
});
