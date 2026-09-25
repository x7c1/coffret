import { useCallback, useEffect, useRef, useState } from 'react';

import {
  getActivity,
  startFill,
  startFreeze,
  startSync,
  type Activity,
  type Catalog,
  type Fill,
  type Freeze,
  type LibraryState,
  type Sync,
} from '@coffret/api';

import {
  canPutAway,
  putAway,
  putAwayFolders,
  servedBy,
  stillOffered,
  NOTHING_DISMISSED,
  type Dismissable,
  type Dismissed,
} from './dismissed';
import { ACTIVITY_INTERVAL_MS, shouldAsk, shouldPoll } from './fill';
import { offeredFolders, stillStanding, type Trouble } from './retry';
import { said } from './useAsked';

/**
 * What the server is doing on its own, followed while there is anything to
 * follow.
 *
 * Polling and not a socket: the whole of what is being followed is a handful of
 * numbers and a status, the server is on this machine, and a connection to keep
 * open would be a second thing to reconnect and reason about for a page that
 * already asks this one question well.
 *
 * When to ask is [`shouldAsk` and `shouldPoll`](./fill), which is where those
 * decisions are stated and tested; this holds the interval and the request. It
 * starts asking the moment the answer could matter and stops the moment it
 * cannot, so an explorer sitting on a folder with nothing in flight makes no
 * request at all — after the one question every page asks as it comes up, which
 * is what a reload comes back to a running or a stopped run by.
 *
 * The answer carries two things that are not work: whether this device still
 * holds the Library open, and how far its catalog has got with the Library. Both
 * are followed here because both arrive here, and because this is the one
 * question a page asks without being told to.
 */
export function useActivity(readerOpen: boolean): {
  fill: Fill | null;
  sync: Sync | null;
  freeze: Freeze | null;
  /**
   * Whether this device still holds the Library open, and `null` until an
   * answer has said.
   *
   * `null` is not a third state of the Library: it is this page not having been
   * told yet, which is what keeps a page that comes up to a shut Library from
   * reading its first answer as a lock that just landed.
   */
  library: LibraryState | null;
  /**
   * How this device's catalog stands with the Library, and `null` until an
   * answer has said.
   *
   * `null` for the reason the state above is `null`: a page that has not been
   * told yet must say nothing about why the Library looks empty, since it does
   * not yet know which kind of emptiness it is looking at.
   */
  catalog: Catalog | null;
  /**
   * The press that was refused and what refused it, and `null` where none was.
   *
   * Only while the offer it answered is still being made; which press that was
   * decides it, and is kept alongside the sentence — see
   * [`stillStanding`](./retry).
   *
   * The press goes out with the sentence rather than being kept back here. The
   * bar has room for one line, but that line is about one of several buttons
   * standing side by side and the sentence in it is the server's about the
   * operation, so the screen needs the press to say which of them was refused.
   */
  trouble: Trouble | null;
  retry: (folder: string) => void;
  retrySync: () => void;
  retryFreeze: (folder: string) => void;
  /**
   * Follow work a drop may have just armed, before any answer has said so.
   *
   * One call for either flow: what a drop arms is a sync or a freeze, and what
   * this page needs from it is the same either way — start asking.
   */
  follow: () => void;
  /**
   * Ask once now, whatever the interval is doing.
   *
   * What the screen's one try-again reaches this through. The question a page
   * asks as it comes up can fail like any other, and a tab whose only ask
   * failed would be one that never again says what is running — while offering
   * nothing to press about it, because this is not one of the screen's regions
   * and has no refusal of its own on the screen.
   */
  recheck: () => void;
  /** What this tab has been told it need not show again. */
  dismissed: Dismissed;
  /** Puts away what the bar is standing beside, whichever kind of notice it is. */
  dismiss: (what: Dismissable) => void;
} {
  const [fill, setFill] = useState<Fill | null>(null);
  const [sync, setSync] = useState<Sync | null>(null);
  const [freeze, setFreeze] = useState<Freeze | null>(null);
  const [library, setLibrary] = useState<LibraryState | null>(null);
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  // A drop arms its flow before it answers, so the server is already running one
  // by the time this page hears the upload landed — and this page has not asked
  // for the activity since. A drop that broke mid-transfer turns this on too: it
  // may have broken after that same arming, and nothing else would start the
  // asking. Without this the first tick would be the one after something else
  // happened to start the polling, which for a drop onto a folder with no reader
  // open is never.
  const [following, setFollowing] = useState(false);
  const [trouble, setTrouble] = useState<Trouble | null>(null);
  const [dismissed, setDismissed] = useState<Dismissed>(NOTHING_DISMISSED);
  const polling = shouldPoll(readerOpen, fill, sync, freeze, catalog) || following;
  // Whether this page has ever asked *and been told*. In a ref rather than in
  // state because nothing on the screen is drawn from it: it is what turns the
  // question every page asks as it comes up into a question asked once (see
  // `shouldAsk`). A question that failed is deliberately not one of them.
  const asked = useRef(false);

  // What one answer does, whichever question it came from: a reload that came
  // back to a stopped freeze has to reach the status bar's line and its "pack
  // again" by the same road a poll's answer does.
  //
  // Out here rather than inside the effect because three askers share it now:
  // the interval, the question a page asks as it comes up, and the screen's
  // try-again.
  const answered = useCallback((activity: Activity) => {
    asked.current = true;
    setFill(activity.fill);
    setSync(activity.sync);
    setFreeze(activity.freeze);
    setLibrary(activity.library);
    setCatalog(activity.catalog);
    // What this tab has put away is put away with one server, and every answer
    // says which one gave it. A name that has changed is a process that was
    // started again — the way a locked Library is opened — and everything held
    // goes with it, because the new process counts its runs from 1 and would
    // otherwise have its first ones hidden by the old one's dismissals.
    //
    // Then the folders, which last only while the server goes on offering them.
    // Taking one up — a button here, or opening a file in it — takes it off
    // these lists, and this is where that shows: a folder thrown away a second
    // time is a fresh offer rather than one put away before it was made.
    setDismissed((away) =>
      stillTold(servedBy(away, activity.server), activity.fill, activity.freeze),
    );
    // Whatever the answer says about the two flows a drop arms, it is an
    // answer: from here on they decide for themselves whether there is
    // anything to follow.
    if (activity.sync?.status !== 'syncing' && activity.freeze?.status !== 'freezing') {
      setFollowing(false);
    }
    // Nothing here about the refusal a press met. What ends one is the offer it
    // answered no longer being made, and that is read off what these lines have
    // just set rather than decided from the answer: a run leaving the state its
    // button was offered from is one of the ways an offer ends, a folder leaving
    // the list of the ones a queue lost is another, and somebody putting the
    // notice away is a third — which nothing here can see at all.
  }, []);

  // A question that failed is not put on the screen. This follows work nobody
  // asked for; a refusal shown here would be reporting the failure of a
  // question the reader never asked, over a screen where everything they did
  // ask for answers for itself. A server that has actually gone says so
  // through the reader and the listing, which are the requests somebody is
  // waiting on.
  //
  // What it must not do is end the asking, and that is why `asked` above is set
  // by an answer rather than by a question: a page whose first ask failed has
  // still been told nothing, so the next reason to ask — the reader opening,
  // the screen's try-again — asks afresh instead of reading a failure as an
  // answer and going quiet for the life of the tab.
  const quietly = () => undefined;

  const recheck = useCallback(() => {
    void getActivity().then(answered, quietly);
  }, [answered]);

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
  }, [polling, answered]);

  // A retry is pressed, so its refusal is answered for: a button that did
  // nothing and said nothing would be the one thing worse than the failure it
  // is offered against.
  //
  // The reply carries the fill as it stands the moment it is armed, which is
  // what takes the stopped state off the screen at once rather than at the next
  // tick — and what starts the polling that follows the rest of it.
  //
  // One folder at a time rather than one request at a time. The bar offers a
  // button per folder — the one that stopped, and one for each the queue lost —
  // and the fill queues what it is asked for by name, so a guard that dropped
  // the second press while the first request was open would be this page
  // throwing away exactly what the server no longer throws away. What it still
  // refuses is the same folder twice over, which is a second press of one
  // button before it has answered.
  const asking = useRef(new Set<string>());
  const retry = useCallback((folder: string) => {
    if (asking.current.has(folder)) {
      return;
    }
    asking.current.add(folder);
    setTrouble(null);
    void startFill(folder)
      .then(
        (activity) => {
          setFill(activity.fill);
          // Through `servedBy` for the reason the poll's answer is: this is an
          // answer like any other, and the process that gave it may not be the
          // one whose runs this tab put away.
          setDismissed((away) =>
            stillOffered(
              servedBy(away, activity.server),
              'fill',
              offeredFolders(activity.fill),
            ),
          );
        },
        (refused: unknown) =>
          setTrouble({ pressed: { flow: 'fill', folder }, said: said(refused) }),
      )
      .finally(() => {
        asking.current.delete(folder);
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
        (activity) => {
          setSync(activity.sync);
          // Through `servedBy` for the reason the other two answers are: this
          // is an answer like any other, and the process that gave it may not
          // be the one whose runs this tab put away. Without it the run this
          // press has just started — counted from 1 by a server that was
          // started again — is measured against a number put away under the
          // old one, and what that hides is the one sentence saying a file
          // somebody dropped is not backed up. No folders are read back from
          // it: a sync offers none, since it walks the device's mappings
          // rather than a folder a screen chose.
          setDismissed((away) => servedBy(away, activity.server));
        },
        (refused: unknown) =>
          setTrouble({ pressed: { flow: 'sync' }, said: said(refused) }),
      )
      .finally(() => {
        syncing.current = false;
      });
  }, []);

  // And the same for the freeze. It takes the folder, unlike the sync: a freeze
  // is of one book, and the one to take up again is the one that stopped — or
  // one of the several its queue lost, which is why this is a book at a time
  // rather than a request at a time, exactly as the fill's is.
  const packing = useRef(new Set<string>());
  const retryFreeze = useCallback((folder: string) => {
    if (packing.current.has(folder)) {
      return;
    }
    packing.current.add(folder);
    setTrouble(null);
    void startFreeze(folder)
      .then(
        (activity) => {
          setFreeze(activity.freeze);
          // The same, for the same reason.
          setDismissed((away) =>
            stillOffered(
              servedBy(away, activity.server),
              'freeze',
              offeredFolders(activity.freeze),
            ),
          );
        },
        (refused: unknown) =>
          setTrouble({ pressed: { flow: 'freeze', folder }, said: said(refused) }),
      )
      .finally(() => {
        packing.current.delete(folder);
      });
  }, []);

  const follow = useCallback(() => setFollowing(true), []);

  // Putting a finished run's line away. What is put away is a run and not a
  // sentence: two runs of one flow can come to the same words, and a screen
  // that remembered the words would swallow the second one's — which for a
  // finding saying a dropped file is not backed up is the whole loss.
  //
  // A run still going is not put away at all. Its counts are still moving, and
  // the next answer would bring the line straight back.
  const dismiss = useCallback(
    (what: Dismissable) => {
      // The folders a queue lost are not a run and have no number, and the runs
      // a later one took the record from are not the run a number would name:
      // what is remembered of either is the folder, until the server stops
      // offering it.
      if (what.kind !== 'line') {
        setDismissed((away) => putAwayFolders(away, what.queue, what.folders));
        return;
      }
      const run = { fill, sync, freeze }[what.flow];
      if (!canPutAway(run)) {
        return;
      }
      setDismissed((away) => putAway(away, what.flow, run.run));
    },
    [fill, sync, freeze],
  );

  // What a press was refused with, for as long as that press is still on offer.
  //
  // Read off the answer on record rather than ended by whichever answer happens
  // to arrive next. The three runs cannot speak for the other buttons: a folder
  // a queue lost is not a run and never has a status, so a refusal from one of
  // those was being taken off the screen by the next poll — inside a second,
  // with the button that met it still standing and nothing left saying why
  // pressing it did nothing.
  const standing = stillStanding(trouble, fill, sync, freeze, dismissed);

  // And let go of once it is not, rather than left lying here unshown. What
  // goes on the screen is settled above; this is what keeps a refusal from
  // coming back afterwards. The offers are made by name and not by occasion —
  // a folder a queue lost is offered again the next time a worker throws it
  // away, and a run that stopped is offered again every time it stops on the
  // same folder — so a sentence kept past the end of the press it answered
  // would stand again under a button nobody had pressed, telling somebody
  // their press was refused when they had made none.
  useEffect(() => {
    if (standing === null) {
      setTrouble(null);
    }
  }, [standing]);

  return {
    fill,
    sync,
    freeze,
    library,
    catalog,
    trouble: standing,
    retry,
    retrySync,
    retryFreeze,
    follow,
    recheck,
    dismissed,
    dismiss,
  };
}

/**
 * What a tab still need not show, given the folders an answer named.
 *
 * Both queues at once, because one answer carries both: a folder put away on
 * either of them is kept only for as long as that queue goes on offering it —
 * by either of the two ways it offers one, which is what
 * [`offeredFolders`](./retry) names together.
 */
function stillTold(
  dismissed: Dismissed,
  fill: Fill | null,
  freeze: Freeze | null,
): Dismissed {
  return stillOffered(
    stillOffered(dismissed, 'fill', offeredFolders(fill)),
    'freeze',
    offeredFolders(freeze),
  );
}
