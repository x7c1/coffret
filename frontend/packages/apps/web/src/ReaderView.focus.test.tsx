import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';

import { getFile } from '@coffret/api';

import type { Page } from './pages';
import { ReaderView } from './ReaderView';

vi.mock('@coffret/api', async (actual) => ({
  ...(await actual<typeof import('@coffret/api')>()),
  getFile: vi.fn(),
}));

const pages: Page[] = [{ path: 'albums/one.jpg', name: 'one.jpg', remote: false }];

beforeEach(() => {
  vi.mocked(getFile).mockResolvedValue(new Blob(['page']));
  URL.createObjectURL = () => 'blob:page';
  URL.revokeObjectURL = () => undefined;
});

afterEach(cleanup);

// The list under the page answers ↑/↓ and `Enter` while it holds the focus, so
// the reader takes it for as long as it is open, and gives it back on closing.
it('holds the focus while open and hands it back on closing', async () => {
  const list = document.createElement('div');
  list.tabIndex = 0;
  document.body.append(list);
  list.focus();

  const { unmount } = render(
    <ReaderView
      pages={pages}
      at={0}
      discarded={0}
      onNavigate={() => undefined}
      onClose={() => undefined}
      onFetching={() => undefined}
      onFetched={() => undefined}
      reconnect={null}
    />,
  );
  await screen.findByAltText('one.jpg');
  expect(document.activeElement).not.toBe(list);
  expect(document.activeElement?.contains(screen.getByAltText('one.jpg'))).toBe(true);

  unmount();
  expect(document.activeElement).toBe(list);
  list.remove();
});
