import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { getFile } from '@coffret/api';

import type { Page } from './pages';
import { ReaderView } from './ReaderView';

// The server, as the reader reaches it: one page's bytes. Everything else the
// package exports is the real thing.
vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getFile: vi.fn(),
}));

const fetched = vi.mocked(getFile);

const pages: Page[] = [
  { path: 'albums/one.jpg', name: 'one.jpg', remote: false },
  { path: 'albums/two.jpg', name: 'two.jpg', remote: false },
];

beforeEach(() => {
  fetched.mockResolvedValue(new Blob(['page']));
  // jsdom has no object URLs, and the reader only needs one to put on the
  // image. Each test file runs in an environment of its own, so these do not
  // reach any other.
  let made = 0;
  URL.createObjectURL = () => `blob:page-${++made}`;
  URL.revokeObjectURL = () => undefined;
});

afterEach(cleanup);

async function open() {
  const onClose = vi.fn();
  render(
    <ReaderView
      pages={pages}
      at={0}
      discarded={0}
      onNavigate={() => undefined}
      onClose={onClose}
      onFetching={() => undefined}
      onFetched={() => undefined}
      reconnect={null}
    />,
  );
  const image = await screen.findByAltText('one.jpg');
  return { image, onClose };
}

// The page is where the pointer rests while somebody reads, and a press there
// is not a request to leave.
it('stays open on a click on the page', async () => {
  const { image, onClose } = await open();
  fireEvent.click(image);
  expect(onClose).not.toHaveBeenCalled();
});

// Anywhere else in the overlay is beside the page, and closes it.
it('closes on a click beside the page', async () => {
  const { onClose } = await open();
  fireEvent.click(screen.getByText(/to turn/));
  expect(onClose).toHaveBeenCalledTimes(1);
});

it('closes on Escape', async () => {
  const { onClose } = await open();
  fireEvent.keyDown(window, { key: 'Escape' });
  expect(onClose).toHaveBeenCalledTimes(1);
});
