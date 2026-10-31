import { useCallback, useEffect, useRef, useState } from 'react';

import {
  getWork,
  startFill,
  startFreeze,
  startSync,
  stopWork,
  type AddUnderWay,
  type StopTarget,
  type Work,
  type Catalog,
  type Delete,
  type Fill,
  type Freeze,
  type FoundByRead,
  type LibraryState,
  type Logged,
  type Reconnect,
  type Sync,
  type TurnTaker,
} from '@coffret/api';

import { stillStanding } from './activity';
import { POLL_INTERVAL_MS, shouldAsk, shouldPoll } from './fill';
import { refusedAsRanOut } from './ranOut';
import type { Pressed, Trouble } from './retry';
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
 * question a page asks without being told to. And it carries the server's log
 * of the work it finished, which the Activity panel lists (spec: LA-12).
 */
export function useWork(readerOpen: boolean): {
  fill: Fill | null;
  sync: Sync | null;
  freeze: Freeze | null;
  /** The latest deletion, running or finished, and `null` where none has run. */
  deletion: Delete | null;
  /**
   * Which flow's run holds this device's pending work, and `null` where nobody
   * does — or where no answer has said.
   */
  turn: TurnTaker | null;
  /**
   * What the server has finished since it started, newest first, and empty
   * until an answer has said.
   */
  log: readonly Logged[];
  /**
   * What the server's reads that armed no fill found, newest first, and empty
   * until an answer has said (spec: LA-33).
   */
  findings: readonly FoundByRead[];
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
   * How the last reconnect stands, and `null` until an answer has said one
   * ran.
   */
  reconnect: Reconnect | null;
  /**
   * The press that was refused and what refused it, and `null` where none was.
   *
   * Only while an entry of the log still offers what it pressed; which press
   * that was decides it, and is kept alongside the sentence — see
   * [`stillStanding`](./activity).
   *
   * The press goes out with the sentence rather than being kept back here: the
   * sentence is the server's about the operation, and the panel says it beside
   * the entry whose Retry was pressed.
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
  /**
   * The adds the server is carrying in, and empty until an answer has said
   * (spec: LA-34).
   */
  adds: readonly AddUnderWay[];
  /**
   * Asks for an add, a freeze or a fill to be stopped (spec: LA-35), and
   * takes the work answer it is met with as every press's is taken: the run's
   * line says it was asked, and the run answers by ending. What refused the
   * asking — a Library locked meanwhile, say — is what the promise rejects
   * with, for the screen to say.
   */
  stop: (target: StopTarget) => Promise<void>;
} {
  const [fill, setFill] = useState<Fill | null>(null);
  const [sync, setSync] = useState<Sync | null>(null);
  const [freeze, setFreeze] = useState<Freeze | null>(null);
  const [deletion, setDeletion] = useState<Delete | null>(null);
  const [turn, setTurn] = useState<TurnTaker | null>(null);
  const [log, setLog] = useState<readonly Logged[]>([]);
  const [findings, setFindings] = useState<readonly FoundByRead[]>([]);
  const [library, setLibrary] = useState<LibraryState | null>(null);
  const [catalog, setCatalog] = useState<Catalog | null>(null);
  const [reconnect, setReconnect] = useState<Reconnect | null>(null);
  const [adds, setAdds] = useState<readonly AddUnderWay[]>([]);
  // A drop arms its flow before it answers, so the server is already running one
  // by the time this page hears the upload landed — and this page has not asked
  // for the work answer since. A drop that broke mid-transfer turns this on too: it
  // may have broken after that same arming, and nothing else would start the
  // asking. Without this the first tick would be the one after something else
  // happened to start the polling, which for a drop onto a folder with no reader
  // open is never.
  const [following, setFollowing] = useState(false);
  const [trouble, setTrouble] = useState<Trouble | null>(null);
  const polling =
    shouldPoll(readerOpen, fill, sync, freeze, catalog, reconnect, library, deletion, adds) ||
    following;
  // Whether this page has ever asked *and been told*. In a ref rather than in
  // state because nothing on the screen is drawn from it: it is what turns the
  // question every page asks as it comes up into a question asked once (see
  // `shouldAsk`). A question that failed is deliberately not one of them.
  const asked = useRef(false);

  // What one answer does, whichever question it came from: a reload that came
  // back to a stopped freeze has to reach the panel's entry and its "pack
  // again" by the same road a poll's answer does.
  //
  // Out here rather than inside the effect because several askers share it: the
  // interval, the question a page asks as it comes up, the screen's try-again,
  // and the presses of a Retry, which answer with the work answer too.
  const answered = useCallback((work: Work) => {
    asked.current = true;
    setFill(work.fill);
    setSync(work.sync);
    setFreeze(work.freeze);
    setDeletion(work.delete);
    setTurn(work.turn);
    setLog(work.log);
    setFindings(work.findings);
    setLibrary(work.library);
    setCatalog(work.catalog);
    setReconnect(work.reconnect);
    setAdds(work.adds);
    // Whatever the answer says about the flows a press arms — the sync and the
    // freeze a drop arms, and a deletion — it is an answer: from here on they
    // decide for themselves whether there is anything to follow.
    if (
      work.sync?.status !== 'syncing' &&
      work.freeze?.status !== 'freezing' &&
      work.delete?.status !== 'deleting'
    ) {
      setFollowing(false);
    }
    // Nothing here about the refusal a press met. What ends one is the offer it
    // answered no longer being made, and that is read off the log these lines
    // have just set rather than decided from the answer.
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
    void getWork().then(answered, quietly);
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
      void getWork().then(answered, quietly);
      return;
    }
    const aborter = new AbortController();
    const ask = () => {
      void getWork(aborter.signal).then(answered, quietly);
    };
    ask();
    const timer = window.setInterval(ask, POLL_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      aborter.abort();
    };
  }, [polling, answered]);

  // A retry is pressed, so its refusal is answered for: a button that did
  // nothing and said nothing would be the one thing worse than the failure it
  // is offered against.
  //
  // The reply is the work answer as it stands the moment the fill is armed,
  // which puts the run under way on the screen at once rather than at the next
  // tick — and starts the polling that follows the rest of it.
  //
  // One folder at a time rather than one request at a time. The panel offers a
  // Retry per folder — each one that stopped, and each the queue lost —
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
      .then(answered, (refused: unknown) =>
        setTrouble(troubleOf({ flow: 'fill', folder }, refused)),
      )
      .finally(() => {
        asking.current.delete(folder);
      });
  }, [answered]);

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
      .then(answered, (refused: unknown) => setTrouble(troubleOf({ flow: 'sync' }, refused)))
      .finally(() => {
        syncing.current = false;
      });
  }, [answered]);

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
      .then(answered, (refused: unknown) =>
        setTrouble(troubleOf({ flow: 'freeze', folder }, refused)),
      )
      .finally(() => {
        packing.current.delete(folder);
      });
  }, [answered]);

  const follow = useCallback(() => setFollowing(true), []);

  // A stop is a request, answered at once with the work answer (spec: LA-35):
  // taken as every press's answer is, so the line says it was asked from this
  // moment rather than from the next tick, and the asking goes on until the
  // run has ended and the log says what it came to.
  const stop = useCallback(
    async (target: StopTarget) => {
      setFollowing(true);
      answered(await stopWork(target));
    },
    [answered],
  );

  // What a press was refused with, for as long as that press is still on offer.
  //
  // Read off the log on record rather than ended by whichever answer happens to
  // arrive next: the next poll brings the same entry back, and taking the
  // refusal away there would leave the Retry that met it standing with nothing
  // saying why pressing it did nothing.
  const standing = stillStanding(trouble, log);

  // And let go of once it is not, rather than left lying here unshown. What
  // goes on the screen is decided above; this is what keeps a refusal from
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
    deletion,
    turn,
    log,
    findings,
    library,
    catalog,
    reconnect,
    trouble: standing,
    retry,
    retrySync,
    retryFreeze,
    follow,
    recheck,
    adds,
    stop,
  };
}

/** One press that was refused, and what refused it. */
function troubleOf(pressed: Pressed, refused: unknown): Trouble {
  return { pressed, said: said(refused), ranOut: refusedAsRanOut(refused) };
}
