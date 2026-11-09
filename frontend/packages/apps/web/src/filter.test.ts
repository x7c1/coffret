import { describe, expect, it } from 'vitest';

import type { FilterAsk, Filtered } from '@coffret/api';

import {
  answeredTree,
  askOf,
  drawnIn,
  filteredOpen,
  matchLine,
  NO_FILTER,
  NO_MATCHES,
  recentCount,
  shapeOf,
  unfiltered,
} from './filter';
import { fileKey, folderKey } from './tree';

function answered(rows: Filtered['rows'], over: Partial<Filtered> = {}): Filtered {
  return { rows, total: rows.length, truncated: false, ...over };
}

it('asks nothing for an empty name with Recent off, and the name as typed otherwise', () => {
  expect(askOf(NO_FILTER)).toBeNull();
  // Recent's count is kept while it is off, and asks for nothing.
  expect(askOf({ ...NO_FILTER, recent: 3 })).toBeNull();
  expect(askOf({ ...NO_FILTER, name: 'ｽﾀｰ ' })).toEqual({ name: 'ｽﾀｰ ' });
  expect(askOf({ ...NO_FILTER, recentOn: true, recent: 5 })).toEqual({ recent: 5 });
  expect(askOf({ name: 'vol', recentOn: true, recent: 2 })).toEqual({ name: 'vol', recent: 2 });
});

it('reads a count of recent books as a whole number from 1', () => {
  expect(recentCount('10')).toBe(10);
  expect(recentCount(' 3 ')).toBe(3);
  for (const typed of ['', '0', '-1', '2.5', 'ten', '1e3', '99999999999999999999']) {
    expect(recentCount(typed), typed).toBeNull();
  }
});

it('shapes an answer into the rows kept and the folders above them', () => {
  const shape = shapeOf(
    answered([
      { path: 'comics/Starfall', kind: 'folder', deposited_at: null },
      { path: 'posters/maps/starfall.png', kind: 'file' },
    ]),
  );
  expect([...shape.answered.keys()]).toEqual([folderKey('comics/Starfall'), fileKey('posters/maps/starfall.png')]);
  expect([...shape.above]).toEqual(['comics', 'posters', 'posters/maps']);
  expect(shape.rootBook).toBe(false);
  expect(drawnIn(shape, folderKey('comics'))).toBe(true);
  expect(drawnIn(shape, folderKey('comics/Starfall'))).toBe(true);
  expect(drawnIn(shape, fileKey('posters/maps/starfall.png'))).toBe(true);
  // Not what a kept folder holds, until it is opened; and not a file by a
  // folder's path.
  expect(drawnIn(shape, fileKey('comics/Starfall/001.jpg'))).toBe(false);
  expect(drawnIn(shape, fileKey('comics'))).toBe(false);
});

it('answers the Library root as a book with no row of its own', () => {
  const shape = shapeOf(answered([{ path: '', kind: 'folder', deposited_at: '2026-01-01T00:00:00Z' }]));
  expect(shape.rootBook).toBe(true);
  expect(shape.answered.size).toBe(0);
  expect(shape.above.size).toBe(0);
});

it('opens the folders above the rows kept, and keeps open what was opened inside a kept folder', () => {
  const shape = shapeOf(answered([{ path: 'comics/Starfall', kind: 'folder', deposited_at: null }]));
  expect([...filteredOpen(shape, new Set())]).toEqual(['comics']);
  expect([...filteredOpen(shape, new Set(['comics', 'comics/Starfall', 'comics/Starfall/vol 1', 'novels']))]).toEqual([
    'comics',
    'comics/Starfall',
    'comics/Starfall/vol 1',
  ]);
});

describe('a filter answered', () => {
  const starfall = answered([
    { path: 'comics/Starfall', kind: 'folder', deposited_at: null },
    { path: 'novels/Star/cover.jpg', kind: 'file', deposited_at: null },
  ]);
  // The same ask answered again after a book was added under a folder closed.
  const starfallAgain = answered([
    { path: 'comics/Starfall', kind: 'folder', deposited_at: null },
    { path: 'novels/Star/cover.jpg', kind: 'file', deposited_at: null },
    { path: 'archive/2024/Stars', kind: 'folder', deposited_at: null },
  ]);
  const byName = { name: 'star' };

  it('on being turned on, opens the folders above every answered row', () => {
    const held = { focus: null, expanded: new Set(['music']) };
    const next = answeredTree(held, shapeOf(starfall), byName, null);
    expect([...next.expanded].sort()).toEqual(['comics', 'novels', 'novels/Star']);
  });

  it('again for the same ask, leaves the folders open as they were', () => {
    // Some opened, some closed: left exactly so.
    const some = { focus: null, expanded: new Set(['comics', 'comics/Starfall']) };
    expect(answeredTree(some, shapeOf(starfallAgain), { name: 'star' }, byName)).toBe(some);
    // Every folder closed: they stay closed, a row new in the answer under a closed folder too.
    const closed = { focus: null, expanded: new Set<string>() };
    expect(answeredTree(closed, shapeOf(starfallAgain), { name: 'star' }, byName)).toBe(closed);
  });

  it('again for the same ask, lets go of the focus only where its row is no longer drawn', () => {
    const held = { focus: folderKey('comics/Starfall'), expanded: new Set<string>() };
    const gone = answered([{ path: 'novels/Star/cover.jpg', kind: 'file', deposited_at: null }]);
    const next = answeredTree(held, shapeOf(gone), byName, byName);
    expect(next.focus).toBeNull();
    expect(next.expanded).toBe(held.expanded);
  });

  it('for other conditions, opens the folders above every answered row again', () => {
    const closed = { focus: null, expanded: new Set<string>() };
    const shape = shapeOf(starfallAgain);
    const above = ['archive', 'archive/2024', 'comics', 'novels', 'novels/Star'];
    const changes: [FilterAsk, FilterAsk][] = [
      // Another name.
      [{ name: 'sta' }, { name: 'star' }],
      // Recent turned on, and turned off.
      [{ name: 'star' }, { name: 'star', recent: 10 }],
      [{ name: 'star', recent: 10 }, { name: 'star' }],
      // Another count.
      [{ recent: 10 }, { recent: 5 }],
    ];
    for (const [drawn, ask] of changes) {
      expect([...answeredTree(closed, shape, ask, drawn).expanded].sort()).toEqual(above);
    }
  });
});

it('brings back the tree as it stood, with the focus left where it was', () => {
  const normal = { focus: folderKey('albums'), expanded: new Set(['albums']) };
  // Nothing focused while filtered: back to the focus before.
  expect(unfiltered(normal, { focus: null, expanded: new Set(['comics']) })).toBe(normal);
  // A row focused while filtered: kept, with the folders above it opened.
  const back = unfiltered(normal, { focus: fileKey('comics/Starfall/001.jpg'), expanded: new Set(['comics']) });
  expect(back.focus).toBe(fileKey('comics/Starfall/001.jpg'));
  expect([...back.expanded]).toEqual(['albums', 'comics', 'comics/Starfall']);
});

it('says how many matched, and how many of how many where the answer stops short', () => {
  expect(matchLine(answered([]))).toBe(NO_MATCHES);
  expect(matchLine(answered([{ path: 'a.jpg', kind: 'file' }]))).toBe('1 match');
  expect(matchLine(answered([], { total: 12 }))).toBe('12 matches');
  expect(matchLine(answered([], { total: 1234 }))).toBe('1,234 matches');
  const first = Array.from({ length: 2000 }, (_, at) => ({ path: `p/${at}.jpg`, kind: 'file' as const }));
  expect(matchLine(answered(first, { total: 8412, truncated: true }))).toBe(
    'Showing the first 2,000 of 8,412 matches — narrow the name to see the rest',
  );
});
