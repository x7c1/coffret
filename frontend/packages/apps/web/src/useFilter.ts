import { useCallback, useEffect, useRef, useState } from 'react';

import { getFilter, type FilterAsk, type Filtered } from '@coffret/api';

import { said } from './useAsked';

/** How long the explorer waits after the last keystroke, or change of the count, before asking. */
const FILTER_WAIT_MS = 250;

/** Where the asking of one filter stands. */
export interface Filtering {
  /**
   * The last answer drawn for the filter on — kept while a newer one is out,
   * or was refused, so the rows stay on the screen meanwhile — and `null`
   * before the first one, or while no filter is on.
   */
  answer: Filtered | null;
  /**
   * The ask `answer` was answered for, which is not the ask on while a newer
   * one is out; `null` where `answer` is.
   */
  answered: FilterAsk | null;
  /** Whether a request for it is out, or about to be sent. */
  asking: boolean;
  /** The sentence the last request was refused with, and `null` where it was not. */
  failed: string | null;
}

const OFF: Filtering = { answer: null, answered: null, asking: false, failed: null };

/**
 * Asks which rows a filtered tree keeps (see [`getFilter`](@coffret/api)),
 * for `ask`, or nothing where it is `null`.
 *
 * Asked `FILTER_WAIT_MS` after `ask` last changed, so that typing a name asks
 * once at the end of it rather than once a keystroke; a request still out
 * when a newer one is sent is let go of, and an answer that arrives for any
 * but the latest is not drawn. `null` is no filter at all: what was answered
 * is dropped, nothing is asked, and a request out is let go of.
 *
 * `reload` asks again for the same filter, after something that changed the
 * Library: the rows it keeps may have moved.
 */
export function useFilter(ask: FilterAsk | null): Filtering & { reload: () => void } {
  const [state, setState] = useState<Filtering>(OFF);
  const [pass, setPass] = useState(0);
  const latest = useRef(ask);
  latest.current = ask;
  // The request out, which the next one sent lets go of.
  const out = useRef<AbortController | null>(null);
  const key = ask === null ? null : JSON.stringify(ask);
  const wanted = useRef(key);
  wanted.current = key;

  useEffect(() => {
    if (key === null) {
      out.current?.abort();
      out.current = null;
      setState((held) => (held === OFF ? held : OFF));
      return;
    }
    setState((held) => (held.asking ? held : { ...held, asking: true }));
    const timer = window.setTimeout(() => {
      const asked = latest.current;
      if (asked === null) {
        return;
      }
      out.current?.abort();
      const aborter = new AbortController();
      out.current = aborter;
      // Drawn only where it is still the request for the filter on.
      const current = () => out.current === aborter && wanted.current === key;
      getFilter(asked, aborter.signal).then(
        (answer) => {
          if (current()) {
            out.current = null;
            setState({ answer, answered: asked, asking: false, failed: null });
          }
        },
        (refused: unknown) => {
          if (aborter.signal.aborted || !current()) {
            return;
          }
          out.current = null;
          setState((held) => ({ ...held, asking: false, failed: said(refused) }));
        },
      );
    }, FILTER_WAIT_MS);
    return () => window.clearTimeout(timer);
  }, [key, pass]);

  // Whatever is still out is let go of with the screen.
  useEffect(() => () => out.current?.abort(), []);

  const reload = useCallback(() => {
    if (latest.current !== null) {
      setPass((made) => made + 1);
    }
  }, []);
  return { ...state, reload };
}
