// The offers of a second attempt the status bar makes, and what one of them was
// refused with, kept free of DOM so it is unit testable.
//
// There are two kinds of offer and they come from different places. One is made
// from a run that stopped — the fill, the sync, the freeze — and stands for as
// long as that run is in that state. The other is made about a folder a queue
// lost, which is nobody's run: nothing counted it, nothing is going to finish
// it, and what ends the offer is somebody taking the folder up.
//
// A refusal one of them met is kept as what it answered rather than as a
// sentence on its own. The difference is the whole of one failure: a refusal
// held as a bare sentence has to be ended by something, and the only thing left
// to end it by is the state of the three runs — which says nothing at all about
// the folder buttons, so a person who pressed one and was refused would watch
// the reason go off the screen at the next poll, with the button they pressed
// still standing and nothing on the screen saying why pressing it did nothing.
// Tied to the press, a refusal lives exactly as long as the offer it answered.

import type { Fill, Freeze, Sync } from '@coffret/api';

import { isPutAway, shownFolders, type Dismissed, type Queue } from './dismissed';

/**
 * Which of the bar's offers a press was.
 *
 * The sync's names no folder, unlike the other two: which folders a sync walks
 * is the device's mappings and never something a screen chooses. The other two
 * name one, and it is not always the folder of the run on record — the folders a
 * queue lost are offered by name beside it.
 */
export type Pressed =
  | { flow: 'sync' }
  | { flow: 'fill'; folder: string }
  | { flow: 'freeze'; folder: string };

/** One press that was refused, and what refused it. */
export interface Trouble {
  /** Which offer this answered. */
  pressed: Pressed;
  /** The refusal's own sentence, as it goes on the screen. */
  said: string;
}

/**
 * Every folder a flow is holding a notice about by name: the ones its queue
 * lost, and the ones its runs stopped on.
 *
 * The two notices are separate — one is about folders nothing started on and the
 * other about runs that got part way — but what ends either is the same thing,
 * somebody taking that folder up, and a dismissal of either lasts exactly as
 * long as the notice it answers. So the two are named together wherever that
 * question is asked.
 *
 * Every folder here has a line; not every one has a button, which is
 * [`offeredAgain`] below. The notice is the wider of the two, and it is this one
 * a dismissal is measured against — a person who put a notice away is owed the
 * quiet whether or not there was anything to press beside it.
 */
export function offeredFolders(run: Fill | Freeze | null): readonly string[] {
  return run === null ? [] : [...run.dropped, ...run.displaced.map((stopped) => stopped.folder)];
}

/**
 * The runs a later one took the record from that the bar is offering a second
 * attempt at, which is not all of them.
 *
 * Every displaced run keeps its line: the folder was asked for and is half here
 * whatever refused it, and saying so is owed. A button is a different promise —
 * that pressing it could change the answer — and [`retryable`] is where that is
 * decided. A run a refused mapped root stopped is mended at a terminal by the
 * gesture its own explanation names and by nothing a browser can press, so while
 * it is the run on record the bar draws no "bring over again" for it.
 *
 * Having the record taken from it is not something that happens to a run's
 * failure — it is somebody else's folder starting — so a run does not become
 * worth repeating by being displaced, and the same question is asked here.
 * Read by the bar to draw the buttons and by [`stillStanding`] to decide how
 * long a refusal of one of them lives, which is the pairing every offer on this
 * bar is held to: a red line under no button tells somebody that something they
 * cannot see was refused.
 */
export function offeredAgain<R extends Fill | Freeze>(
  displaced: readonly R[],
): readonly R[] {
  return displaced.filter((run) => retryable(run));
}

/**
 * Whether a stopped background run can be helped by making the same request.
 *
 * A refused mapped root needs the recovery named in its visible explanation;
 * repeating the run cannot change the mapping. Older servers did not send a
 * structured reason, so their stopped runs retain the ordinary retry.
 */
export function retryable(run: Fill | Sync | Freeze | null): boolean {
  return run?.status === 'stopped' && run.stopped?.reason !== 'refused_root';
}

/**
 * The refusal, where the offer it answered is still being made, and `null`
 * where it is not.
 *
 * Derived from the answer on record rather than ended by an event, so that
 * there is one rule and no moment at which a refusal outlives what it is about.
 * A sentence in red beside a line saying a folder is being brought over is the
 * bar contradicting itself, and one standing under no button at all is a person
 * being told that something they cannot see was refused.
 *
 * The dismissals count, because a notice somebody has put away takes its buttons
 * with it: the offer is gone from the screen, so the refusal it met is about
 * nothing a person can look at.
 */
export function stillStanding(
  trouble: Trouble | null,
  fill: Fill | null,
  sync: Sync | null,
  freeze: Freeze | null,
  dismissed: Dismissed,
): Trouble | null {
  if (trouble === null) {
    return null;
  }
  const pressed = trouble.pressed;
  const offered =
    pressed.flow === 'sync'
      ? retryable(sync) && !isPutAway(dismissed, 'sync', sync)
      : offersFolder(
          pressed.flow === 'fill' ? fill : freeze,
          pressed.flow,
          pressed.folder,
          dismissed,
        );
  return offered ? trouble : null;
}

/**
 * Whether that folder's offer is the second attempt at the run on record, as
 * against one of the folders that run's queue lost.
 *
 * Both offers make the same request, so nothing the server does turns on which
 * of them was pressed — but two readers ask this and they have to answer it
 * alike. It is half of how long a refusal of that folder lives, below, and it
 * is which button's words the bar names that refusal by. Written out twice,
 * one of them would eventually call a press "bring over again" after the other
 * had let its refusal go, and the line left standing would name a button that
 * is no longer on the bar.
 *
 * `null` is no run and so no second attempt: what a queue lost is read off the
 * run's own lists, and there are none without a run.
 */
export function offersAgain(run: Fill | Freeze | null, folder: string): boolean {
  return retryable(run) && run?.folder === folder;
}

/**
 * Whether the bar is still offering that folder of that queue, by either of the
 * two ways it offers one.
 *
 * A run that stopped is offered again under the folder it stopped on, and every
 * folder the queue lost is offered under its own — as is every run that stopped
 * before a later one took the record from it. The second attempt and the folders
 * that never had a first are separate offers, which is why a press has to say
 * which folder it was rather than only which flow.
 *
 * Named apart from the queue's own [`stillOffered`](./dismissed), which is what
 * the server is still holding out: this is what the bar is drawing, dismissals
 * and all.
 */
function offersFolder(
  run: Fill | Freeze | null,
  queue: Queue,
  folder: string,
  dismissed: Dismissed,
): boolean {
  if (run === null) {
    return false;
  }
  return (
    (offersAgain(run, folder) && !isPutAway(dismissed, queue, run)) ||
    shownFolders(dismissed, queue, pressable(run)).includes(folder)
  );
}

/**
 * The folders of that flow a button is standing under, as against the ones it
 * merely has a notice about.
 *
 * What the queue lost, all of it — nothing ran on those folders, so nothing
 * about them is beyond taking up again — and the displaced runs
 * [`offeredAgain`] admits. Narrower than [`offeredFolders`], and deliberately:
 * that one measures how long a dismissal lasts, and this one how long a refusal
 * does, which is exactly as long as the button it answered.
 */
function pressable(run: Fill | Freeze): readonly string[] {
  return [
    ...run.dropped,
    ...offeredAgain<Fill | Freeze>(run.displaced).map((stopped) => stopped.folder),
  ];
}
