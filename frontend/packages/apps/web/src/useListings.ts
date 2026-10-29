import { useCallback, useEffect, useRef, useState } from 'react';

import { getListing, isLockedRefusal, type Listing } from '@coffret/api';

import { refusedAsRanOut } from './ranOut';
import { said, type Asked } from './useAsked';

/**
 * The listings of every folder the tree shows, each asked for once it is
 * shown and let go of once it is not.
 *
 * [`useAsked`](./useAsked) for many folders at once, on the same terms: each
 * listing is in one of the three states and every one of them leaveable, a
 * refusal carries the server's own sentence, and asking again keeps the rows
 * on hand while the answer is out — a listing asked again because a fetch
 * made one of its rows present must not blank the tree under the reader.
 *
 * One request per folder per asking. A folder that leaves `wanted` — closed,
 * or its parent closed — has its request let go of and its answer dropped, so
 * a folder opened again is asked again rather than drawn from an answer that
 * may be any age.
 *
 * `reload` asks again for the folders named, among those wanted, or for every
 * one of them where none is named.
 */
export function useListings(wanted: readonly string[]): {
  listing: (folder: string) => Asked<Listing> | undefined;
  reload: (folders?: readonly string[]) => void;
} {
  const [states, setStates] = useState<ReadonlyMap<string, Asked<Listing>>>(() => new Map());
  // What has been asked for each folder: the request in flight, to let go of.
  const asked = useRef(new Map<string, AbortController>());
  // The folders to ask again at the next pass, or every one of them.
  const stale = useRef<Set<string> | 'all'>(new Set());
  const [pass, setPass] = useState(0);
  // What decides when to look again is the folders themselves, joined; the
  // array is a new one every render.
  const latest = useRef(wanted);
  latest.current = wanted;
  const key = wanted.join('\u0000');

  useEffect(() => {
    const want = new Set(latest.current);
    const again = stale.current;
    stale.current = new Set();
    for (const [folder, aborter] of asked.current) {
      if (!want.has(folder)) {
        aborter.abort();
        asked.current.delete(folder);
      }
    }
    setStates((held) => {
      if ([...held.keys()].every((folder) => want.has(folder))) {
        return held;
      }
      return new Map([...held].filter(([folder]) => want.has(folder)));
    });
    for (const folder of want) {
      const askedBefore = asked.current.get(folder);
      if (askedBefore !== undefined && again !== 'all' && !again.has(folder)) {
        continue;
      }
      askedBefore?.abort();
      const aborter = new AbortController();
      asked.current.set(folder, aborter);
      setStates((held) => {
        const before = held.get(folder);
        if (before?.status === 'ready') {
          return held;
        }
        return new Map(held).set(folder, { status: 'loading' });
      });
      getListing(folder, aborter.signal).then(
        (value) => {
          if (!aborter.signal.aborted) {
            setStates((held) => new Map(held).set(folder, { status: 'ready', value }));
          }
        },
        (refused: unknown) => {
          if (aborter.signal.aborted) {
            return;
          }
          setStates((held) =>
            new Map(held).set(folder, {
              status: 'failed',
              message: said(refused),
              ranOut: refusedAsRanOut(refused),
              locked: isLockedRefusal(refused),
            }),
          );
        },
      );
    }
  }, [key, pass]);

  // Everything still out is let go of with the screen — and forgotten, so a
  // screen mounted again asks afresh rather than waiting on requests it let go.
  useEffect(() => {
    const held = asked.current;
    return () => {
      for (const aborter of held.values()) {
        aborter.abort();
      }
      held.clear();
    };
  }, []);

  const reload = useCallback((folders?: readonly string[]) => {
    const marked = stale.current;
    if (folders === undefined) {
      stale.current = 'all';
    } else if (marked !== 'all') {
      for (const folder of folders) {
        marked.add(folder);
      }
    }
    setPass((made) => made + 1);
  }, []);

  const listing = useCallback((folder: string) => states.get(folder), [states]);
  return { listing, reload };
}
