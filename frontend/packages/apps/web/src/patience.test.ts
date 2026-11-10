import { expect, it, vi } from 'vitest';

import { Refusal } from '@coffret/api';

import { askedThroughDrops, droppedByNetwork } from './patience';

const PAUSES = [1, 1];

function dropped(): Refusal {
  return new Refusal('unreachable', 0, 'the coffret server did not answer');
}

// Only a request that got no answer at all: an answer whose status arrived
// and whose body broke off was answered, and a refusal is the server's word.
it('counts as dropped only a request with no status at all', () => {
  expect(droppedByNetwork(dropped())).toBe(true);
  expect(droppedByNetwork(new Refusal('unreachable', 502, 'broke off'))).toBe(false);
  expect(droppedByNetwork(new Refusal('server', 500, 'refused'))).toBe(false);
  expect(droppedByNetwork(new Error('not a refusal'))).toBe(false);
});

it('asks again after a drop, and settles as the attempt that was answered', async () => {
  const ask = vi.fn().mockRejectedValueOnce(dropped()).mockResolvedValue('listed');
  await expect(askedThroughDrops(ask, new AbortController().signal, PAUSES)).resolves.toBe('listed');
  expect(ask).toHaveBeenCalledTimes(2);
});

it('asks no more than one attempt past its pauses, and rejects as the last did', async () => {
  const last = dropped();
  const ask = vi.fn().mockRejectedValueOnce(dropped()).mockRejectedValueOnce(dropped()).mockRejectedValue(last);
  await expect(askedThroughDrops(ask, new AbortController().signal, PAUSES)).rejects.toBe(last);
  expect(ask).toHaveBeenCalledTimes(PAUSES.length + 1);
});

it('does not ask again after a refusal', async () => {
  const refused = new Refusal('locked', 423, 'the Library is locked');
  const ask = vi.fn().mockRejectedValue(refused);
  await expect(askedThroughDrops(ask, new AbortController().signal, PAUSES)).rejects.toBe(refused);
  expect(ask).toHaveBeenCalledTimes(1);
});

// A load let go of — its folder closed, its screen gone — stops asking where
// it is, in the middle of a pause included.
it('asks nothing more once its signal is aborted', async () => {
  const aborter = new AbortController();
  const ask = vi.fn(() => {
    queueMicrotask(() => aborter.abort());
    return Promise.reject(dropped());
  });
  await expect(askedThroughDrops(ask, aborter.signal, [60_000])).rejects.toBeInstanceOf(Refusal);
  expect(ask).toHaveBeenCalledTimes(1);
});
