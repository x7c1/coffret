import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import type { Freeze, Sync } from '@coffret/api';

import { useRunReloads } from './runReloads';

afterEach(cleanup);

function sync(status: 'syncing' | 'done', run = 1): Sync {
  return { run, status, stopped: null, added: 1, findings: [], step: null, waits: false };
}

function freeze(status: 'freezing' | 'done', run = 1): Freeze {
  return {
    run,
    status,
    stopped: null,
    folder: 'books/vol-1',
    packs: 1,
    entries: 3,
    findings: [],
    step: null,
    waits: false,
    waiting: [],
    discarded: [],
    displaced: [],
  };
}

/** The hook over `runs`, with the two reloads it asks for counted. */
function watched(runs: { sync: Sync | null; freeze: Freeze | null }) {
  const listing = vi.fn();
  const folders = vi.fn();
  const hook = renderHook(
    ({ sync, freeze }) => useRunReloads(sync, freeze, listing, folders),
    { initialProps: runs },
  );
  listing.mockClear();
  folders.mockClear();
  return { ...hook, listing, folders };
}

// A one-by-one add can make a folder, and its commit is the moment the Library
// has one to name — so the tree is asked again with the listing.
it('asks for the folders as well as the listing when a sync changes status', () => {
  const { rerender, listing, folders } = watched({ sync: sync('syncing'), freeze: null });

  rerender({ sync: sync('done'), freeze: null });

  expect(listing).toHaveBeenCalledTimes(1);
  expect(folders).toHaveBeenCalledTimes(1);
});

// A second sync over before any poll saw it run is still a commit.
it('asks again for a later sync that reads done after done', () => {
  const { rerender, listing, folders } = watched({ sync: sync('done', 1), freeze: null });

  rerender({ sync: sync('done', 2), freeze: null });

  expect(listing).toHaveBeenCalledTimes(1);
  expect(folders).toHaveBeenCalledTimes(1);
});

it('asks for both when a freeze changes status, and for nothing while nothing moves', () => {
  const { rerender, listing, folders } = watched({ sync: null, freeze: freeze('freezing') });

  rerender({ sync: null, freeze: freeze('freezing') });
  expect(listing).not.toHaveBeenCalled();
  expect(folders).not.toHaveBeenCalled();

  rerender({ sync: null, freeze: freeze('done') });
  expect(listing).toHaveBeenCalledTimes(1);
  expect(folders).toHaveBeenCalledTimes(1);
});
