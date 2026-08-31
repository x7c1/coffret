import { expect, it } from 'vitest';

import { ancestry, nest } from './tree';

/** One node as `name(child, child)`, so a case can state a whole tree. */
function drawn(nodes: ReturnType<typeof nest>): string {
  return nodes
    .map((node) =>
      node.children.length === 0 ? node.name : `${node.name}(${drawn(node.children)})`,
    )
    .join(', ');
}

it('nests the flat list by its separators', () => {
  const tree = nest(['albums', 'albums/2026', 'albums/2026/08', 'books', 'books/some-novel']);

  expect(drawn(tree)).toBe('albums(2026(08)), books(some-novel)');
  expect(tree[0].path).toBe('albums');
  expect(tree[0].children[0].path).toBe('albums/2026');
});

// EP-3 order is what the server answered in and the only order every device
// agrees on, so the tree keeps it rather than sorting again. `Z` is 0x5a and
// `a` is 0x61: a locale-aware sort would put these the other way round.
it('keeps the order the server answered in', () => {
  const tree = nest(['albums', 'albums/Zurich', 'albums/aachen']);

  expect(drawn(tree)).toBe('albums(Zurich, aachen)');
});

// A Library has folders only where a current Entry stands under one, so an empty
// Library has none — which is an empty tree and not an error.
it('nests an empty Library into an empty tree', () => {
  expect(nest([])).toEqual([]);
});

it('makes a parent the list did not name', () => {
  const tree = nest(['albums/2026/08']);

  expect(drawn(tree)).toBe('albums(2026(08))');
  expect(tree[0].children[0].children[0].path).toBe('albums/2026/08');
});

it('lists every folder on the way down to one, outermost first', () => {
  expect(ancestry('albums/2026/08')).toEqual(['albums', 'albums/2026', 'albums/2026/08']);
  expect(ancestry('albums')).toEqual(['albums']);
  expect(ancestry('')).toEqual([]);
});
