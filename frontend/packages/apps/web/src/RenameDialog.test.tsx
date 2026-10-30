import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { RenameDialog, type RenameAnswer } from './RenameDialog';

afterEach(cleanup);

it('fills in the current name, selected, and says a refusal under the field', async () => {
  const onRename = vi.fn(async (): Promise<RenameAnswer> => ({ refused: 'a name is needed' }));
  render(<RenameDialog name="Vol. 03" onRename={onRename} onClose={vi.fn()} />);
  const field = screen.getByLabelText<HTMLInputElement>('new name');
  expect(field.value).toBe('Vol. 03');
  expect([field.selectionStart, field.selectionEnd]).toEqual([0, 'Vol. 03'.length]);
  fireEvent.change(field, { target: { value: '' } });
  fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
  await waitFor(() => expect(screen.getByRole('alert').textContent).toBe('a name is needed'));
  expect(onRename).toHaveBeenCalledWith('', false);
});

it('says the files that would leave this device, and goes ahead on Rename again', async () => {
  const onRename = vi.fn(
    async (_typed: string, acknowledged: boolean): Promise<RenameAnswer> =>
      acknowledged ? null : { leaving: '3 files (3.0 kB) on this device will be removed from it' },
  );
  render(<RenameDialog name="series" onRename={onRename} onClose={vi.fn()} />);
  fireEvent.change(screen.getByLabelText('new name'), { target: { value: 'shelf' } });
  fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
  await waitFor(() => expect(screen.getByRole('dialog').textContent).toContain('3 files (3.0 kB)'));
  fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
  await waitFor(() => expect(onRename).toHaveBeenLastCalledWith('shelf', true));
});

it('marks the field for a name the name rules refuse, and only for one', async () => {
  const onRename = vi.fn(async (): Promise<RenameAnswer> => ({ refused: 'there is already something called shelf here' }));
  render(<RenameDialog name="series" onRename={onRename} onClose={vi.fn()} />);
  const field = screen.getByLabelText('new name');
  expect(field.getAttribute('aria-invalid')).toBe('false');
  fireEvent.change(field, { target: { value: 'shelf' } });
  fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
  await waitFor(() => expect(field.getAttribute('aria-invalid')).toBe('true'));
  fireEvent.change(field, { target: { value: 'shelf 2' } });
  expect(field.getAttribute('aria-invalid')).toBe('false');
  expect(screen.queryByRole('alert')).toBeNull();
});

it('says a refusal that is not about the name in the body, the field left unmarked', async () => {
  const onRename = vi.fn(async (): Promise<RenameAnswer> => ({ blocked: 'series/vol-1/page.jpg is still uploading' }));
  render(<RenameDialog name="series" onRename={onRename} onClose={vi.fn()} />);
  const field = screen.getByLabelText('new name');
  fireEvent.change(field, { target: { value: 'shelf' } });
  fireEvent.click(screen.getByRole('button', { name: 'Rename' }));
  await waitFor(() => expect(screen.getByRole('alert').textContent).toBe('series/vol-1/page.jpg is still uploading'));
  expect(field.getAttribute('aria-invalid')).toBe('false');
  fireEvent.change(field, { target: { value: 'shelf 2' } });
  expect(screen.queryByRole('alert')).toBeNull();
});
