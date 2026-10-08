import { expect, it, vi } from 'vitest';

import { Refusal, type Reconnecting, type ReconnectState } from '@coffret/api';

import {
  askToReconnect,
  consentLink,
  grantRenewed,
  offersReconnect,
  ranOut,
  reconnectLine,
  refusedAsRanOut,
} from './reconnect';

/** One press, with everything it reaches out to recorded. */
function pressing(ask: () => Promise<Reconnecting>) {
  const said: (string | null)[] = [];
  const trouble: (string | null)[] = [];
  const open = vi.fn();
  const follow = vi.fn();
  return {
    said,
    trouble,
    open,
    follow,
    run: () =>
      askToReconnect({
        ask,
        open,
        line: (line) => said.push(line),
        trouble: (line) => trouble.push(line),
        follow,
      }),
  };
}

// The one refusal a reconnect answers is told apart from every other Storage
// refusal by its reason, wherever it arrived.
it('knows a permission that ran out from Storage not answering', () => {
  expect(ranOut({ kind: 'storage', reason: 'unauthenticated' })).toBe(true);
  expect(ranOut({ kind: 'storage', reason: null })).toBe(false);
  expect(ranOut({ kind: 'declined', reason: 'key_lost' })).toBe(false);
  expect(ranOut(null)).toBe(false);

  expect(
    refusedAsRanOut(new Refusal('storage', 502, 'no longer accepted', 'unauthenticated')),
  ).toBe(true);
  expect(refusedAsRanOut(new Refusal('storage', 502, 'did not answer'))).toBe(false);
  expect(refusedAsRanOut(new Error('not a refusal'))).toBe(false);
});

// The press: the server is asked, the page it answers with is opened in a tab
// of its own, its sentence is said, and the work answer is followed — which is
// where the flow's ending will land.
it('opens the consent page the server answers with and follows the work answer', async () => {
  const press = pressing(() =>
    Promise.resolve({ url: 'https://consent.example/', message: 'answer the consent page' }),
  );

  await press.run();

  expect(press.open).toHaveBeenCalledWith('https://consent.example/');
  expect(press.said).toEqual([null, 'answer the consent page']);
  expect(press.trouble).toEqual([null]);
  expect(press.follow).toHaveBeenCalledOnce();
});

// A press the server refused opens nothing and follows nothing: the refusal is
// the sentence beside the button, and the button is still there.
it('says what refused a press and opens nothing', async () => {
  const press = pressing(() =>
    Promise.reject(new Refusal('locked', 423, 'the Library is locked')),
  );

  await press.run();

  expect(press.open).not.toHaveBeenCalled();
  expect(press.follow).not.toHaveBeenCalled();
  expect(press.trouble).toEqual([null, 'the Library is locked']);
});

// While a flow waits there is nothing to press; every ending of it offers the
// button again, with what it came to beside it — except a renewal, which the
// notice says instead.
it('offers the button again after every ending and says each one', () => {
  const at = (state: ReconnectState) => ({ state, message: `it is ${state}` });

  expect(offersReconnect(null)).toBe(true);
  expect(offersReconnect(at('waiting'))).toBe(false);
  for (const state of ['refused', 'timed_out', 'failed'] as const) {
    expect(offersReconnect(at(state))).toBe(true);
    expect(reconnectLine(at(state))).toBe(`it is ${state}`);
  }
  expect(reconnectLine(at('waiting'))).toBe('it is waiting');
  expect(reconnectLine(at('renewed'))).toBeNull();
  expect(reconnectLine(null)).toBeNull();
});

// What reloads the listing: a flow that was not renewed and now is. A page
// coming up to a server whose last reconnect was renewed has just asked for
// the listing, and a renewal it was already told of is not news.
it('reads a renewal landing as news once', () => {
  expect(grantRenewed('waiting', 'renewed')).toBe(true);
  expect(grantRenewed('refused', 'renewed')).toBe(true);
  expect(grantRenewed(null, 'renewed')).toBe(false);
  expect(grantRenewed('renewed', 'renewed')).toBe(false);
  expect(grantRenewed('waiting', 'waiting')).toBe(false);
  expect(grantRenewed('waiting', 'refused')).toBe(false);
});

// The page the press was answered with stays reachable as a link for as long
// as the server is listening behind it, because a browser may have blocked the
// tab the press opened and the button is gone while the flow waits.
it('links to the consent page only while the flow waits on it', () => {
  const at = (state: ReconnectState) => ({ state, message: `it is ${state}` });
  const page = 'https://consent.example/';

  expect(consentLink(at('waiting'), page)).toBe(page);
  expect(consentLink(at('waiting'), null)).toBeNull();
  expect(consentLink(null, page)).toBeNull();
  for (const state of ['renewed', 'refused', 'timed_out', 'failed'] as const) {
    expect(consentLink(at(state), page)).toBeNull();
  }
});
