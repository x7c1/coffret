import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { NewFolderDialog } from './NewFolderDialog';

afterEach(cleanup);

// The folder the new one goes into is set apart in the heading, so a name
// such as "Series A" is not read as part of the sentence; the dialog is still
// read out as one plain question.
it('sets the folder it makes the new one in apart from the words of its heading', () => {
  render(<NewFolderDialog parent="comics/Series A" onPrepare={vi.fn()} onClose={vi.fn()} />);
  const dialog = screen.getByRole('dialog', { name: 'New folder in comics/Series A' });
  const place = within(dialog).getByTestId('place');
  expect(place.textContent).toBe('comics/Series A');
  expect(place.closest('h2')).not.toBeNull();
  cleanup();
  render(<NewFolderDialog parent="" onPrepare={vi.fn()} onClose={vi.fn()} />);
  expect(screen.getByRole('dialog', { name: 'New folder in the Library' })).toBeTruthy();
  expect(screen.getByTestId('place').textContent).toBe('Library');
});
