import { expect, it } from 'vitest';

import { ROWS_AHEAD, scrollAhead, type ScrollGeometry } from './scrollAhead';

/** Rows 20px tall in an area 200px tall, the top 30px under the headings. */
const ROW = 20;

function at(row: number, scrollTop: number): ScrollGeometry {
  return {
    top: row * ROW,
    bottom: (row + 1) * ROW,
    scrollTop,
    height: 200,
    covered: 30,
    margin: ROWS_AHEAD * ROW,
  };
}

it('keeps the focused row three rows from the bottom edge', () => {
  // Row 10 ends at 220; three rows below it is 280, which an area of 200
  // scrolled to 80 ends at.
  expect(scrollAhead(at(10, 0))).toBe(80);
  // The next row down moves the list by one row.
  expect(scrollAhead(at(11, 80))).toBe(100);
});

it('keeps the focused row three rows from the top edge, under the headings', () => {
  // Row 10 starts at 200; three rows above it is 140, which is under the
  // headings unless the list is scrolled to 110.
  expect(scrollAhead(at(10, 300))).toBe(110);
  // Never past the top of the list.
  expect(scrollAhead(at(1, 100))).toBe(0);
  expect(scrollAhead(at(0, 0))).toBeNull();
});

it('does not scroll where the row is three rows from either edge already', () => {
  // Row 5 spans 100–120 with the list unscrolled: 40 above it is clear of the
  // headings, and 60 below it is inside the 200.
  expect(scrollAhead(at(5, 0))).toBeNull();
  expect(scrollAhead(at(6, 0))).toBeNull();
  expect(scrollAhead(at(10, 80))).toBeNull();
});
