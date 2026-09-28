import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { FolderTree } from './FolderTree';

// jsdom lays nothing out and has no scrollIntoView of its own, so each case
// that needs one puts a recorder in its place and this takes it away again.
const unscrolled = Element.prototype.scrollIntoView;
afterEach(() => {
  cleanup();
  Element.prototype.scrollIntoView = unscrolled;
});

function drawAt(current: string) {
  return (
    <FolderTree
      folders={['albums', 'albums/2026', 'albums/2026/08', 'books']}
      pending={[]}
      current={current}
      onOpen={() => undefined}
      onNewFolder={() => undefined}
    />
  );
}

/** The row a tree-name button stands in, which is what gets scrolled. */
function rowOf(name: string): Element | null {
  return screen.getByTitle(name).parentElement;
}

// A folder restored from a URL several components deep opens every branch above
// it, and the row that marks it has to be brought into view as well — otherwise
// it is below the fold with nothing on the screen to say where it is.
it('scrolls the current folder into view, and moves with it', () => {
  const scrolled: Element[] = [];
  const scroll = vi.fn(function (this: Element) {
    scrolled.push(this);
  });
  Element.prototype.scrollIntoView = scroll;

  const { rerender } = render(drawAt('albums/2026/08'));
  expect(scroll).toHaveBeenCalledWith({ block: 'nearest' });
  expect(scrolled).toEqual([rowOf('08')]);

  rerender(drawAt('books'));
  expect(scrolled).toEqual([rowOf('08'), rowOf('books')]);
});
