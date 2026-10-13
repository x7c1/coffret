import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { Region, TRY_AGAIN } from './Region';
import { UNLOCK, type UnlockOffer } from './StatusBar';
import type { Asked } from './useAsked';

afterEach(cleanup);

function offer(over: Partial<UnlockOffer> = {}): UnlockOffer {
  return { asking: false, said: null, refused: null, ask: vi.fn(), ...over };
}

function mount(state: Asked<string>, unlock: UnlockOffer) {
  const onRetry = vi.fn();
  render(
    <Region state={state} onRetry={onRetry} reconnect={null} unlock={unlock}>
      {(value) => <p>{value}</p>}
    </Region>,
  );
  return { onRetry };
}

const lockedRefusal = 'the Library is locked — give the Passphrase to unlock it';

// A region refused by the lock offers the unlock rather than a retry that
// would earn the same refusal however often it was pressed.
it('offers the unlock in place of a retry where the lock refused it', () => {
  const unlock = offer();
  const { onRetry } = mount({ status: 'failed', message: lockedRefusal, locked: true }, unlock);

  expect(screen.getByText(lockedRefusal)).toBeTruthy();
  expect(screen.queryByRole('button', { name: TRY_AGAIN })).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: UNLOCK }));
  expect(unlock.ask).toHaveBeenCalledTimes(1);
  expect(onRetry).not.toHaveBeenCalled();
});

// What the ask came to stands under it, so that a server with no window to
// bring forward says where to unlock instead of the screen looping.
it('says what the last unlock came to, the refusal over the answer', () => {
  mount(
    { status: 'failed', message: lockedRefusal, locked: true },
    offer({ said: 'the Passphrase window is open on the desktop' }),
  );
  expect(screen.getByText('the Passphrase window is open on the desktop')).toBeTruthy();
  cleanup();

  mount(
    { status: 'failed', message: lockedRefusal, locked: true },
    offer({ said: 'asked', refused: 'this server has no window — run `coffret unlock`' }),
  );
  expect(screen.getByText('this server has no window — run `coffret unlock`')).toBeTruthy();
  expect(screen.queryByText('asked')).toBeNull();
});

// A press still waiting for its answer is not pressed again.
it('holds the unlock while a press is waiting', () => {
  mount({ status: 'failed', message: lockedRefusal, locked: true }, offer({ asking: true }));
  expect((screen.getByRole('button', { name: UNLOCK }) as HTMLButtonElement).disabled).toBe(true);
});

// Any other refusal is asked again, as before.
it('offers to try again where anything else refused it', () => {
  const unlock = offer();
  const { onRetry } = mount({ status: 'failed', message: 'the server is not answering' }, unlock);

  expect(screen.queryByRole('button', { name: UNLOCK })).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: TRY_AGAIN }));
  expect(onRetry).toHaveBeenCalledTimes(1);
  expect(unlock.ask).not.toHaveBeenCalled();
});

it('draws what it was given once it is ready', () => {
  mount({ status: 'ready', value: 'the rows' }, offer());
  expect(screen.getByText('the rows')).toBeTruthy();
});
