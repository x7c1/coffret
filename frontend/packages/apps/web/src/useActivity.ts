import { useCallback, useEffect, useRef, useState } from 'react';

import {
  getActivity,
  startFill,
  startFreeze,
  startSync,
  type Activity,
  type Fill,
  type Freeze,
  type Sync,
} from '@coffret/api';

import { ACTIVITY_INTERVAL_MS, shouldAsk, shouldPoll } from './fill';
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
 * When to ask is [`shouldAsk` and `shouldPoll`](./fill), which is where those
 * decisions are stated and tested; this holds the interval and the request. It
 * starts asking the moment the answer could matter and stops the moment it
 * cannot, so an explorer sitting on a folder with nothing in flight makes no
 * request at all — after the one question every page asks as it comes up, which
 * is what a reload comes back to a stopped run by.
 */
export function useActivity(readerOpen: boolean): {
  fill: Fill | null;
  sync: Sync | null;
  freeze: Freeze | null;
  /** What a retry was refused with, and `null` where none was. */
  trouble: string | null;
  retry: (folder: string) => void;
  retrySync: () => void;
  retryFreeze: (folder: string) => void;
  /**
   * Follow work a drop has just armed, before any answer has said so.
   *
   * One call for either flow: what a drop arms is a sync or a freeze, and what
   * this page needs from it is the same either way — start asking.
   */
  follow: () => void;
} {
  const [fill, setFill] = useState<Fill | null>(null);
  const [sync, setSync] = useState<Sync | null>(null);
  const [freeze, setFreeze] = useState<Freeze | null>(null);
  // A drop arms its flow before it answers, so the server is already running one
  // by the time this page hears the upload landed — and this page has not asked
  // for the activity since. Without this the first tick would be the one after
  // something else happened to start the polling, which for a drop onto a folder
  // with no reader open is never.
  const [following, setFollowing] = useState(false);
  const [trouble, setTrouble] = useState<string | null>(null);
  const polling = shouldPoll(readerOpen, fill, sync, freeze) || following;
  // Whether this page has ever asked. In a ref rather than in state because
  // nothing on the screen is drawn from it: it is what turns the question every
  // page asks as it comes up into a question asked once (see `shouldAsk`).
  const asked = useRef(false);

  // Both questions, because they are one request and one answer: the interval's
  // tick, and the single one this page asks as it comes up.
  //
  // The interval is started and stopped by whether to be polling at all, and by
  // nothing else: an effect that also watched the answer would tear the timer
  // down and build it again on every tick.
  useEffect(() => {
    if (!shouldAsk(asked.current, polling)) {
      return;
    }
    asked.current = true;
    // What one answer does, whichever question it came from: a reload that came
    // back to a stopped freeze has to reach the status bar's line and its "pack
    // again" by the same road a poll's answer does.
    const answered = (activity: Activity) => {
      setFill(activity.fill);
      setSync(activity.sync);
      setFreeze(activity.freeze);
      // Whatever the answer says about the two flows a drop arms, it is an
      // answer: from here on they decide for themselves whether there is
      // anything to follow.
      if (activity.sync?.status !== 'syncing' && activity.freeze?.status !== 'freezing') {
        setFollowing(false);
      }
      // The refusal a retry met belongs to the stopped run the button was
      // offered from, and lives exactly as long as that state does. Once the
      // server says the run is out of it — the retry landed after all, or
      // opening a file armed a fresh one — the sentence is about nothing on
      // the screen, and a refusal standing in red beside a line saying a
      // folder is being brought over is the bar contradicting itself.
      //
      // All three, because there are three buttons and one line for what
      // refused whichever was pressed. Watching only the fill would take a
      // refused "pack again" off the screen at the next tick — while the
      // button that met it is still standing, offered from a freeze that is
      // still stopped, and the person has been told nothing about why
      // pressing it did nothing.
      const stopped =
        activity.fill?.status === 'stopped' ||
        activity.sync?.status === 'stopped' ||
        activity.freeze?.status === 'stopped';
      if (!stopped) {
        setTrouble(null);
      }
    };
    // A question that failed is not put on the screen. This follows work nobody
    // asked for; a refusal shown here would be reporting the failure of a
    // question the reader never asked, over a screen where everything they did
    // ask for answers for itself. A server that has actually gone says so
    // through the reader and the listing, which are the requests somebody is
    // waiting on.
    const quietly = () => undefined;
    if (!polling) {
      // The one question at the start. There is nothing running to follow, so
      // no interval is started and nothing is torn down: the request is left to
      // answer, since aborting it would leave a page that came up having asked
      // and heard nothing. What it hears may itself be a reason to poll — a run
      // somebody armed at the command line — and this effect runs again for it.
      void getActivity().then(answered, quietly);
      return;
    }
    const aborter = new AbortController();
    const ask = () => {
      void getActivity(aborter.signal).then(answered, quietly);
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

  // And the same for the freeze. It takes the folder, unlike the sync: a freeze
  // is of one book, and the one to take up again is the one that stopped.
  const packing = useRef(false);
  const retryFreeze = useCallback((folder: string) => {
    if (packing.current) {
      return;
    }
    packing.current = true;
    setTrouble(null);
    void startFreeze(folder)
      .then(
        (activity) => setFreeze(activity.freeze),
        (refused: unknown) => setTrouble(said(refused)),
      )
      .finally(() => {
        packing.current = false;
      });
  }, []);

  const follow = useCallback(() => setFollowing(true), []);

  return { fill, sync, freeze, trouble, retry, retrySync, retryFreeze, follow };
}
