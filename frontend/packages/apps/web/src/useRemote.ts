import { useCallback, useEffect, useRef, useState } from 'react';

import { isRefusal } from '@coffret/api';

/** One thing the server was asked for, in whichever state the asking is in. */
export type Remote<T> =
  | { status: 'loading' }
  | { status: 'ready'; value: T }
  | { status: 'failed'; message: string };

/**
 * Asks the server for something, and re-asks when `key` changes.
 *
 * Three states and every one of them leaveable. A request that failed carries
 * the refusal's own sentence and is retried by calling `reload` — which is what
 * this exists for: the spike logged its one failure to the console and left the
 * screen saying "loading" for as long as the tab was open.
 *
 * The previous answer is dropped the moment `key` changes, so a folder that is
 * still loading never shows the last one's rows. A `reload` of the *same* key
 * keeps them: that is a refresh of something already on the screen — a listing
 * asked again because a fetch made one of its rows present — and blanking it
 * would take the reader down with it and throw away the pages it holds.
 */
export function useRemote<T>(
  ask: (signal: AbortSignal) => Promise<T>,
  key: string,
): { state: Remote<T>; reload: () => void } {
  const [state, setState] = useState<Remote<T>>({ status: 'loading' });
  const [attempt, setAttempt] = useState(0);

  // The caller writes its closure inline, so it is a new function every render;
  // what decides when to ask again is `key`, and the latest closure is what the
  // asking uses.
  const latest = useRef(ask);
  latest.current = ask;
  const asked = useRef<string | null>(null);

  useEffect(() => {
    const aborter = new AbortController();
    const elsewhere = asked.current !== key;
    asked.current = key;
    setState((held) =>
      elsewhere || held.status !== 'ready' ? { status: 'loading' } : held,
    );
    latest.current(aborter.signal).then(
      (value) => {
        if (!aborter.signal.aborted) {
          setState({ status: 'ready', value });
        }
      },
      (refused: unknown) => {
        if (aborter.signal.aborted) {
          return;
        }
        setState({ status: 'failed', message: said(refused) });
      },
    );
    return () => aborter.abort();
  }, [key, attempt]);

  const reload = useCallback(() => setAttempt((made) => made + 1), []);
  return { state, reload };
}

/**
 * What went wrong, as a sentence to put on the screen.
 *
 * A refusal's message is the server's own and is written to be read; anything
 * else reaching here is this client's own mistake, and says so rather than
 * showing whatever a stray value stringifies to.
 */
export function said(refused: unknown): string {
  if (isRefusal(refused)) {
    return refused.message;
  }
  console.error('the explorer failed at something that is not a refusal', refused);
  return 'the explorer could not finish that';
}
