import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';

import { Place } from './Dialog';
import { COLOR } from './theme';

afterEach(cleanup);

// The folder a dialog names is drawn with the tree's folder icon, in the same
// two colours: filled pale blue inside a deeper blue outline.
it('draws the folder it names with the tree’s pale blue folder icon', () => {
  render(<Place path="books/vol-1" />);
  const place = screen.getByTestId('place');
  expect(place.textContent).toBe('books/vol-1');
  const icon = place.querySelector('svg') as SVGElement;
  expect(icon.getAttribute('fill')).toBe(COLOR.folderFill);
  expect(icon.getAttribute('stroke')).toBe(COLOR.folderStroke);
});
