import { useCallback, useEffect, useRef, useState } from 'react';

import { getActivity, startFill, startSync, type Fill, type Sync } from '@coffret/api';

import { ACTIVITY_INTERVAL_MS, shouldPoll } from './fill';
import { said } from './useRemote';

/**
 * What the server is doing on its own, followed while there is anything to
 * follow.
 *
 * Polling and not a socket: the whole of what is being followed is four numbers
 * and a status, the server is on this machine, and a connection to keep open
 * would be a second thing to reconnect and reason about for a page that already
 * asks this one question well.
 *
 * When to ask is [`shouldPoll`](./fill), which is where that decision is stated
 * and tested; this holds the interval and the request. It starts asking the
 * moment the answer could matter and stops the moment it cannot, so an explorer
 * sitting on a folder with nothing in flight makes no request at all.
 */
export function useActivity(readerOpen: boolean): {
  fill: Fill | null;
  sync: Sync | null;
  /** What a retry was refused with, and `null` where none was. */
  trouble: string | null;
  retry: (folder: string) => void;
  retrySync: () => void;
  /** Follow a sync a drop has just armed, before any answer has said so. */
  follow: () => void;
} {
  const [fill, setFill] = useState<Fill | null>(null);
  const [sync, setSync] = useState<Sync | null>(null);
  // A drop arms a sync before it answers, so the server is already running one
  // by the time this page hears the upload landed — and this page has not asked
  // for the activity since. Without this the first tick would be the one after
  // something else happened to start the polling, which for a drop onto a folder
  // with no reader open is never.
  const [following, setFollowing] = useState(false);
  const [trouble, setTrouble] = useState<string | null>(null);
  const polling = shouldPoll(readerOpen, fill, sync) || following;

  // The interval is started and stopped by whether to be polling at all, and by
  // nothing else: an effect that also watched the answer would tear the timer
  // down and build it again on every tick.
  useEffect(() => {
    if (!polling) {
      return;
    }
    const aborter = new AbortController();
    const ask = () => {
      void getActivity(aborter.signal).then(
        (activity) => {
          setFill(activity.fill);
          setSync(activity.sync);
          // Whatever the answer says about the sync, it is an answer: from here
          // on the sync itself decides whether there is anything to follow.
          if (activity.sync?.status !== 'syncing') {
            setFollowing(false);
          }
          // The refusal a retry met belongs to the failed fill the button was
          // offered from, and lives exactly as long as that state does. Once the
          // server says the fill is out of it — the retry landed after all, or
          // opening a file armed a fresh one — the sentence is about nothing on
          // the screen, and a refusal standing in red beside a line saying a
          // folder is being brought over is the bar contradicting itself.
          if (activity.fill?.status !== 'stopped') {
            setTrouble(null);
          }
        },
        // A poll that failed is not put on the screen. This follows work nobody
        // asked for; a refusal shown here would be reporting the failure of a
        // question the reader never asked, over a screen where everything they
        // did ask for answers for itself. A server that has actually gone says
        // so through the reader and the listing, which are the requests somebody
        // is waiting on.
        () => undefined,
      );
    };
    ask();
    const timer = window.setInterval(ask, ACTIVITY_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      aborter.abort();
    };
  }, [polling]);

  // A retry is pressed, so its refusal is answered for: a button that did
  // nothing and said nothing would be the one thing worse than the failure it
  // is offered against.
  //
  // The reply carries the fill as it stands the moment it is armed, which is
  // what takes the stopped state off the screen at once rather than at the next
  // tick — and what starts the polling that follows the rest of it.
  const asking = useRef(false);
  const retry = useCallback((folder: string) => {
    if (asking.current) {
      return;
    }
    asking.current = true;
    setTrouble(null);
    void startFill(folder)
      .then(
        (activity) => setFill(activity.fill),
        (refused: unknown) => setTrouble(said(refused)),
      )
      .finally(() => {
        asking.current = false;
      });
  }, []);

  // The same, for the sync. It takes no folder: which folders a sync walks is
  // the device's mappings and never something a screen chooses.
  const syncing = useRef(false);
  const retrySync = useCallback(() => {
    if (syncing.current) {
      return;
    }
    syncing.current = true;
    setTrouble(null);
    void startSync()
      .then(
        (activity) => setSync(activity.sync),
        (refused: unknown) => setTrouble(said(refused)),
      )
      .finally(() => {
        syncing.current = false;
      });
  }, []);

  const follow = useCallback(() => setFollowing(true), []);

  return { fill, sync, trouble, retry, retrySync, follow };
}
