import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { Region, TRY_AGAIN } from './Region';
import type { Asked } from './useAsked';

afterEach(cleanup);

function mount(state: Asked<string>) {
  const onRetry = vi.fn();
  render(
    <Region state={state} onRetry={onRetry} reconnect={null}>
      {(value) => <p>{value}</p>}
    </Region>,
  );
  return { onRetry };
}

// A refusal is said, and asked again on a press.
it('says what refused it and offers to try again', () => {
  const { onRetry } = mount({ status: 'failed', message: 'the server is not answering' });

  expect(screen.getByText('the server is not answering')).toBeTruthy();
  fireEvent.click(screen.getByRole('button', { name: TRY_AGAIN }));
  expect(onRetry).toHaveBeenCalledTimes(1);
});

it('says it is loading until the answer lands', () => {
  mount({ status: 'loading' });
  expect(screen.getByText('loading…')).toBeTruthy();
});

it('draws what it was given once it is ready', () => {
  mount({ status: 'ready', value: 'the rows' });
  expect(screen.getByText('the rows')).toBeTruthy();
});
