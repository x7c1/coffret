// What the explorer's tree is filtered by, kept for this device by the server
// (spec: LA-43), so that a reload of the page, or the desktop app started
// again, opens the explorer filtered exactly as it was left.

import { useCallback, useEffect, useRef, useState } from 'react';

import { getFilterSettings, setFilterSettings } from '@coffret/api';

import { NO_FILTER, type FilterSettings } from './filter';

/** Where the settings are kept for this device: the server's two calls about them. */
export interface KeptFilter {
  read: (signal: AbortSignal) => Promise<FilterSettings>;
  write: (settings: FilterSettings) => Promise<FilterSettings>;
}

/** The server's, which is what everything but a test keeps the settings in. */
const ON_THE_SERVER: KeptFilter = {
  read: (signal) => getFilterSettings(signal),
  write: (settings) => setFilterSettings(settings),
};

/**
 * How long typing in the name has to pause before the name is kept: long
 * enough that a word typed at speed is one request, short enough that a
 * reload straight after is unlikely to beat it.
 */
export const NAME_PAUSE_MS = 500;

/** A change to the settings: the next settings, or how to make them from the ones held. */
export type FilterChange = FilterSettings | ((held: FilterSettings) => FilterSettings);

export interface KeptFilterState {
  /** The settings the page is filtered by. */
  settings: FilterSettings;
  /** Changes them, and keeps the change (see [`useKeptFilter`]). */
  change: (next: FilterChange) => void;
  /** The name input was left: a name still waiting for typing to pause is kept now. */
  nameLeft: () => void;
}

/**
 * The filter settings, starting from no filter and replaced by the ones kept
 * for this device once the server answers — so the first filtered tree the
 * page draws is the kept one — and every change kept again.
 *
 * - A change to Recent or its count is kept at once, with the name as it
 *   stands; a change to the name alone is kept once typing pauses for
 *   [`NAME_PAUSE_MS`], or as the field is left, rather than at every key.
 * - Writes go one after another, so the last change made is the last kept.
 * - A change made before the kept settings are answered wins over them: what
 *   somebody set on this page is not undone by an answer that was late.
 * - A failure to read or to keep is said to the console and leaves the
 *   filter on screen as it is. While the Library is locked the server, and
 *   the host in front of it, refuse both (spec: LA-43, LA-14); `open` going
 *   from `false` back to `true` — the unlock — reads again where nothing has
 *   been read or set yet, and keeps again what a refusal left unkept.
 */
export function useKeptFilter(open: boolean, kept: KeptFilter = ON_THE_SERVER): KeptFilterState {
  const [settings, setSettings] = useState<FilterSettings>(NO_FILTER);
  // What is held, for a change made from it and for a write to send, without
  // waiting for the render that `settings` arrives in.
  const held = useRef<FilterSettings>(NO_FILTER);
  // Whether the settings stand for something: read from the server, or set
  // on this page. Until then an answer may replace them.
  const settled = useRef(false);
  // Whether the last write was refused, so that the unlock keeps it again.
  const unkept = useRef(false);
  const pause = useRef<ReturnType<typeof setTimeout> | null>(null);
  const writing = useRef<Promise<unknown>>(Promise.resolve());

  const write = useCallback(() => {
    if (pause.current !== null) {
      clearTimeout(pause.current);
      pause.current = null;
    }
    const sent = held.current;
    writing.current = writing.current.then(() =>
      kept.write(sent).then(
        () => {
          if (held.current === sent) {
            unkept.current = false;
          }
        },
        (refused: unknown) => {
          unkept.current = true;
          console.warn('the explorer could not keep the filter settings for this device', refused);
        },
      ),
    );
  }, [kept]);

  const change = useCallback(
    (next: FilterChange) => {
      const before = held.current;
      const after = typeof next === 'function' ? next(before) : next;
      if (sameSettings(after, before)) {
        return;
      }
      held.current = after;
      settled.current = true;
      setSettings(after);
      if (after.recentOn === before.recentOn && after.recent === before.recent) {
        // The name alone: kept once typing pauses.
        if (pause.current !== null) {
          clearTimeout(pause.current);
        }
        pause.current = setTimeout(write, NAME_PAUSE_MS);
      } else {
        write();
      }
    },
    [write],
  );

  const nameLeft = useCallback(() => {
    if (pause.current !== null) {
      write();
    }
  }, [write]);

  // A name still waiting to be kept as the hook goes is kept as it goes. A
  // reload or a closed window unmounts nothing, so a name typed within the
  // pause before one is not kept (see `NAME_PAUSE_MS`).
  useEffect(
    () => () => {
      if (pause.current !== null) {
        write();
      }
    },
    [write],
  );

  useEffect(() => {
    if (!open) {
      return;
    }
    if (settled.current) {
      if (unkept.current) {
        write();
      }
      return;
    }
    const aborter = new AbortController();
    kept.read(aborter.signal).then(
      (answer) => {
        if (settled.current) {
          return;
        }
        settled.current = true;
        // The same as on screen already — no filter, on a device that never
        // kept one — is left as it stands, so nothing drawn from it is drawn
        // again.
        if (!sameSettings(answer, held.current)) {
          held.current = answer;
          setSettings(answer);
        }
      },
      (refused: unknown) => {
        if (!aborter.signal.aborted) {
          console.warn('the explorer could not read the filter settings kept for this device', refused);
        }
      },
    );
    return () => aborter.abort();
  }, [open, kept, write]);

  return { settings, change, nameLeft };
}

/** Whether two settings filter alike: the same name, Recent and count. */
function sameSettings(one: FilterSettings, other: FilterSettings): boolean {
  return one.name === other.name && one.recentOn === other.recentOn && one.recent === other.recent;
}
