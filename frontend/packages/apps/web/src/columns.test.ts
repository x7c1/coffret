import { expect, it } from 'vitest';

import { columnWidth, type Measured } from './columns';

/** A measure that draws a heading's letters 6 wide and a value's 8. */
function measure(text: string, measured: Measured): number {
  return text.length * (measured === 'heading' ? 6 : 8);
}

it('draws a column as wide as its widest value, where that is wider than its heading', () => {
  // `Size` is 24; `999.9 MB` is 64; the padding is added once.
  expect(columnWidth('Size', ['1.0 kB', '999.9 MB', '12 B'], measure, 20)).toBe(84);
});

it('draws a column as wide as its heading, where that is wider than every value', () => {
  // `Modified` is 48; the widest value is 40.
  expect(columnWidth('Modified', ['—', '12345'], measure, 20)).toBe(68);
  expect(columnWidth('Modified', [], measure, 20)).toBe(68);
});

it('measures each string in its own font, and rounds a fraction of a pixel up', () => {
  const seen: [string, Measured][] = [];
  const width = columnWidth(
    'Size',
    ['1.0 kB'],
    (text, measured) => {
      seen.push([text, measured]);
      return measured === 'heading' ? 20.2 : 30.4;
    },
    0,
  );
  expect(seen).toEqual([
    ['Size', 'heading'],
    ['1.0 kB', 'value'],
  ]);
  expect(width).toBe(31);
});
