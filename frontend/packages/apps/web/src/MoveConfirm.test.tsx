import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { MoveConfirm } from './MoveConfirm';
import type { MoveQuestion } from './moving';

afterEach(cleanup);

const PLAIN: MoveQuestion = {
  pairs: [{ from: 'a/Vol. 03', to: 'Series A/Vol. 03' }],
  title: 'Move “Vol. 03” to “Series A”?',
  lines: ['12 files will be given a new path in the Library — nothing is uploaded'],
  leaving: '12 files (1.2 GB) on this device will be removed from it; opening them fetches them again',
  refused: [],
  moves: true,
};

it('asks before moving, with Cancel focused first, and says what leaves this device', () => {
  const onChoose = vi.fn();
  render(<MoveConfirm question={PLAIN} onChoose={onChoose} />);
  const dialog = screen.getByRole('dialog', { name: PLAIN.title });
  expect(dialog.textContent).toContain(PLAIN.leaving);
  expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Cancel' }));
  fireEvent.click(screen.getByRole('button', { name: 'Move' }));
  expect(onChoose).toHaveBeenCalledWith(true);
});

it('offers Cancel alone for a refused move, and says why', () => {
  const onChoose = vi.fn();
  render(
    <MoveConfirm
      question={{ ...PLAIN, lines: [], leaving: null, refused: ['Series A/Vol. 03 is there already'], moves: false }}
      onChoose={onChoose}
    />,
  );
  expect(screen.getByRole('alert').textContent).toBe('Series A/Vol. 03 is there already');
  expect(screen.getAllByRole('button').map((button) => button.textContent)).toEqual(['Cancel']);
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onChoose).toHaveBeenCalledWith(false);
});
