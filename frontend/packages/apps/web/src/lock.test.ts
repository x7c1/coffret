import { expect, it, vi } from 'vitest';

import { Refusal, type Locked } from '@coffret/api';

import { askToLock } from './lock';

/** One lock, with everything it reaches out to recorded in the order it went. */
function locking(ask: () => Promise<Locked>) {
  const order: string[] = [];
  const trouble: (string | null)[] = [];
  return {
    order,
    trouble,
    run: () =>
      askToLock({
        ask,
        discard: () => order.push('discard'),
        reload: () => order.push('reload'),
        trouble: (line) => trouble.push(line),
      }),
  };
}

// The whole of what the gesture does: the pages this device decrypted are given
// up, and then the screen asks its questions again — which is what puts the
// server's own sentence about the Passphrase where the rows were.
//
// The discard comes first, and the order is the point: the plaintext is gone
// the moment the key is, rather than when a request the network is still
// carrying comes back.
it('gives up what this device decrypted and then asks the screen again', async () => {
  const ask = vi.fn(() => Promise.resolve({ locked: true }));
  const run = locking(ask);

  await run.run();

  expect(ask).toHaveBeenCalledTimes(1);
  expect(run.order).toEqual(['discard', 'reload']);
  expect(run.trouble).toEqual([null]);
});

// A lock that did not happen takes nothing back. The pages are still protected
// by a key the server still holds, so the reading somebody is in the middle of
// is left exactly as it was — and the sentence says outright that the Library
// is open, because one who read only the reason could walk away from a machine
// they believe is closed.
it('revokes nothing where the lock was refused, and says the Library is open', async () => {
  const run = locking(() =>
    Promise.reject(
      new Refusal('storage', 502, "the Library's Storage did not answer"),
    ),
  );

  await run.run();

  expect(run.order).toEqual([]);
  expect(run.trouble).toEqual([
    null,
    "the Library is still open on this device — the Library's Storage did not answer",
  ]);
});

// The press after that one. The sentence a refused lock leaves says the Library
// is open, and it is the one sentence on this screen somebody could act on by
// walking away — so a lock that takes has to take it down with it, or the screen
// would say the Library is open in the notice and refuse everything below it
// because it is shut.
it('takes down the sentence about an open Library when a later lock takes', async () => {
  let asked = 0;
  const run = locking((): Promise<Locked> => {
    asked += 1;
    return asked === 1
      ? Promise.reject(new Refusal('storage', 502, "the Library's Storage did not answer"))
      : Promise.resolve({ locked: true });
  });

  await run.run();
  await run.run();

  expect(run.order).toEqual(['discard', 'reload']);
  expect(run.trouble.at(-1)).toBeNull();
});
