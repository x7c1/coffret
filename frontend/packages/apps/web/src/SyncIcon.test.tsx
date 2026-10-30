import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { CHANGES_WAITING, CHECKING, LOOKING } from './refresh';
import { SyncIcon } from './SyncIcon';
import { COLOR } from './theme';

afterEach(() => {
  cleanup();
});

/** The colour the browser resolves `hex` to, as an inline style reads back. */
function resolved(hex: string): string {
  const probe = document.createElement('span');
  probe.style.color = hex;
  return probe.style.color;
}

// Plain: nothing is known to be waiting, so the icon is drawn in the bar's own
// colour and called by what it does.
it('is drawn plain and called "Check for changes" when nothing is waiting', () => {
  render(<SyncIcon running={false} waiting={false} onPress={() => undefined} />);

  const icon = screen.getByRole('button', { name: CHECKING });
  expect(icon.getAttribute('title')).toBe(CHECKING);
  expect(icon.style.color).toBe('inherit');
  expect(icon.querySelector('circle')).toBeNull();
});

// Changes waiting: the accent colour and a dot, and a name that says so — the
// name still opens with what the control does, so it is found by that.
it('says changes are waiting in its colour and in its name', () => {
  render(<SyncIcon running={false} waiting={true} onPress={() => undefined} />);

  const icon = screen.getByRole('button', { name: CHANGES_WAITING });
  expect(CHANGES_WAITING.startsWith(CHECKING)).toBe(true);
  expect(icon.getAttribute('title')).toBe(CHANGES_WAITING);
  expect(icon.style.color).toBe(resolved(COLOR.accent));
  expect(icon.querySelector('circle')).not.toBeNull();
});

// Pressing it is the catch-up.
it('runs the catch-up when pressed', () => {
  const onPress = vi.fn();
  render(<SyncIcon running={false} waiting={true} onPress={onPress} />);

  fireEvent.click(screen.getByRole('button', { name: CHANGES_WAITING }));

  expect(onPress).toHaveBeenCalledTimes(1);
});

// While the catch-up runs the arrows turn, and a second press is not taken.
it('turns while the catch-up runs and cannot be pressed again', () => {
  const onPress = vi.fn();
  render(<SyncIcon running={true} waiting={true} onPress={onPress} />);

  const icon = screen.getByRole('button', { name: LOOKING });
  expect(icon).toHaveProperty('disabled', true);
  expect(icon.querySelector('animateTransform')).not.toBeNull();
  fireEvent.click(icon);
  expect(onPress).not.toHaveBeenCalled();
});
