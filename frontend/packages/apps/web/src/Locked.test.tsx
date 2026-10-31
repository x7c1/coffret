import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { LOCKED_HEADING, LOCKED_LINES, LockedScreen, UNLOCK, type UnlockOffer } from './Locked';
import { COLOR } from './theme';

afterEach(cleanup);

function offer(over: Partial<UnlockOffer> = {}): UnlockOffer {
  return { asking: false, said: null, refused: null, ask: vi.fn(), ...over };
}

// The locked state: a heading, what the lock means and what to do about it,
// and the one button — drawn quietly, with nothing in a refusal's colour.
it('says the Library is locked, in two lines, with Unlock under them', () => {
  const unlock = offer();
  render(<LockedScreen unlock={unlock} />);

  expect(screen.getByRole('heading', { name: LOCKED_HEADING }).style.fontSize).toBe('18px');
  for (const line of LOCKED_LINES) {
    expect(screen.getByText(line).parentElement?.style.color).toBe(COLOR.dim);
  }
  fireEvent.click(screen.getByRole('button', { name: UNLOCK }));
  expect(unlock.ask).toHaveBeenCalledTimes(1);
  expect(document.body.innerHTML).not.toContain(COLOR.refused);
  expect(document.body.textContent).not.toMatch(/\btab\b/i);
});

// What the ask came to stands under it, so that a server with no window to
// bring forward says where to unlock instead of the screen looping.
it('says what the last unlock came to, the refusal over the answer', () => {
  render(<LockedScreen unlock={offer({ said: 'the Passphrase window is open on the desktop' })} />);
  expect(screen.getByText('the Passphrase window is open on the desktop')).toBeTruthy();
  cleanup();

  render(<LockedScreen unlock={offer({ said: 'asked', refused: 'this server has no window — run `coffret unlock`' })} />);
  expect(screen.getByText('this server has no window — run `coffret unlock`').style.color).toBe(COLOR.dim);
  expect(screen.queryByText('asked')).toBeNull();
  expect(document.body.innerHTML).not.toContain(COLOR.refused);
});

// A press still waiting for its answer is not pressed again.
it('holds Unlock while a press is waiting', () => {
  render(<LockedScreen unlock={offer({ asking: true })} />);
  expect((screen.getByRole('button', { name: UNLOCK }) as HTMLButtonElement).disabled).toBe(true);
});
