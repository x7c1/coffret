import { expect, it } from 'vitest';

import { onPicture } from './picture';

const box = { left: 100, top: 0, width: 800, height: 600 };

// A tall page fitted into a wide box leaves bands either side, and those are
// beside the page rather than on it.
it('tells the page from the bands a fitted page leaves beside it', () => {
  const tall = { width: 300, height: 600 };
  // The page is drawn 300 wide in the middle of the box: 350 to 650.
  expect(onPicture(box, tall, 500, 300)).toBe(true);
  expect(onPicture(box, tall, 360, 10)).toBe(true);
  expect(onPicture(box, tall, 340, 300)).toBe(false);
  expect(onPicture(box, tall, 660, 300)).toBe(false);
});

// And a wide page in a tall box leaves them above and below.
it('tells the page from the bands above and below a wide one', () => {
  const wide = { width: 1600, height: 600 };
  // Drawn 800 by 300, from 150 to 450 down the box.
  expect(onPicture(box, wide, 500, 300)).toBe(true);
  expect(onPicture(box, wide, 500, 100)).toBe(false);
  expect(onPicture(box, wide, 500, 500)).toBe(false);
});

// Until the page's size is known the whole element counts as the page.
it('takes a page of unknown size to fill its box', () => {
  expect(onPicture(box, { width: 0, height: 0 }, 110, 5)).toBe(true);
  expect(onPicture(box, { width: 0, height: 0 }, 50, 5)).toBe(false);
});
