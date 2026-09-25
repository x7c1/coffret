import type { Fill, Freeze, Library, Sync } from '@coffret/api';

import {
  canPutAway,
  isPutAway,
  shownFolders,
  shownRuns,
  type Dismissable,
  type Dismissed,
} from './dismissed';
import {
  droppedBooksLine,
  droppedLine,
  fillLine,
  freezeLine,
  stoppedBooksLine,
  stoppedLine,
  syncLine,
} from './fill';
import { ASKING } from './refresh';
import { offeredAgain, offersAgain, retryable, type Pressed, type Trouble } from './retry';
import { COLOR } from './theme';
import type { Remote } from './useRemote';

/**
 * Which Library this is, along the bottom.
 *
 * The name and the provider, and a line while something is happening — a file
 * being brought over, a drop's files landing in the folder, the sync carrying
 * them in, a book being packed — with the offer of a second attempt where one
 * of those stopped.
 *
 * And two controls that are not about work already running. Asking the Library
 * what is new is one: it is here because it is about the Library as a whole
 * rather than about the folder on the screen, which is what the status bar
 * names — and because nothing else on this page would ever ask, there being no
 * polling of the remote head.
 *
 * Locking the server is the other, and it stands beside the Library's name
 * because that is what it is about: this Library, on this device, stops being
 * open. It is one word and no dialogue — there is no session screen here and
 * there is not meant to be — and what it costs is stated where it is offered,
 * because it is not undone from a browser: the Passphrase is typed at a
 * terminal, so the way back is starting the server again.
 *
 * Nothing else. There is no management screen in this release, and a status bar
 * that grew one would be the place it happened by accident.
 */
export function StatusBar({
  library,
  fetching,
  adding,
  fill,
  sync,
  freeze,
  trouble,
  dismissed,
  onDismiss,
  onRetryFill,
  onRetrySync,
  onRetryFreeze,
  onLock,
  locking,
  refresh,
}: {
  library: Remote<Library>;
  fetching: string | null;
  /** The drop whose files are still being written into the folder, if one is. */
  adding: string | null;
  /** What the server is bringing over on its own, if anything. */
  fill: Fill | null;
  /** What the server is carrying into the Library on its own, if anything. */
  sync: Sync | null;
  /** What the server is packing into the Library on its own, if anything. */
  freeze: Freeze | null;
  /**
   * The press that was refused and what refused it, if one was.
   *
   * The press and not only the sentence. The bar can be holding out half a
   * dozen buttons at once — a second attempt at each of the three runs, and one
   * for every folder a queue lost — while the refusal is written by the server
   * about the operation rather than about the button, so the sentence on its
   * own leaves which of them did nothing to be guessed at.
   */
  trouble: Trouble | null;
  /** What this tab has been told it need not show again. */
  dismissed: Dismissed;
  /** Puts away whatever the one dismiss button is standing beside. */
  onDismiss: (what: Dismissable) => void;
  onRetryFill: (folder: string) => void;
  onRetrySync: () => void;
  onRetryFreeze: (folder: string) => void;
  /** Ends this server's hold on the Master Key until the Passphrase opens it. */
  onLock: () => void;
  /** Whether that is being asked for right now. */
  locking: boolean;
  /**
   * Asking the Library what is new, and what the last asking came to.
   *
   * One value rather than four props, because the four are one control: the
   * button, whether it is busy, what it found, and what refused it.
   */
  refresh: {
    /** Whether a refresh is running right now. */
    running: boolean;
    /** What the last one came to, and `null` before any has run. */
    said: string | null;
    /** What refused the last one, and `null` where none was. */
    refused: string | null;
    ask: () => void;
  };
}) {
  // One line for what is in flight, and there is an order to who takes it. The
  // drop's own line comes first, because it is the only one about a request this
  // page is still making; the freeze and the sync it arms take over from it, and
  // are put above the fill because they are what somebody just asked for by
  // dropping. The freeze leads the sync because it is the larger of the two: a
  // book takes minutes where a photograph takes a moment, and the two are never
  // armed by one drop.
  //
  // Each candidate carries the colour it is drawn in, because the colour belongs
  // to whoever the line belongs to: a line about the sync drawn in red because a
  // fill stopped some minutes ago would be the bar colouring one thing by the
  // state of another — and it has room to say only the one.
  //
  // A run whose line has been put away is passed over here rather than drawn
  // dim or drawn empty: what a dismissal buys is the line beneath it, and a bar
  // that went on holding the place of a sentence nobody wants would buy
  // nothing. The order is unchanged — it is the same candidates, minus the ones
  // somebody has finished reading. The offers of a second attempt are made from
  // these same three, so a line put away takes its offer with it: a bare "bring
  // over again" standing where no sentence says what failed is a button with
  // nothing behind it.
  const freezeShown = isPutAway(dismissed, 'freeze', freeze) ? null : freeze;
  const syncShown = isPutAway(dismissed, 'sync', sync) ? null : sync;
  const fillShown = isPutAway(dismissed, 'fill', fill) ? null : fill;
  // And the same for the two lists of lost folders, which are put away by name
  // rather than by run: they are nobody's run. A folder put away takes its
  // button with it, because the line and the buttons beside it are one notice.
  const booksLost = shownFolders(dismissed, 'freeze', droppedBooks(freeze));
  const foldersLost = shownFolders(dismissed, 'fill', droppedFolders(fill));
  // And the runs each flow stopped on and then had the record taken from, which
  // are put away by name for the same reason: the number a dismissal spends is
  // the flow's latest, and none of these is that. Putting the running line away
  // must not take them with it — a book that stopped is still a folder of pages
  // outside the Library, whatever the book after it did.
  const booksStopped = shownRuns(dismissed, 'freeze', freeze?.displaced ?? []);
  const foldersStopped = shownRuns(dismissed, 'fill', fill?.displaced ?? []);
  const line =
    shown(adding, COLOR.text) ??
    shown(freezeLine(freezeShown), toneOf(freeze?.status, freeze?.findings.length ?? 0)) ??
    shown(syncLine(syncShown), toneOf(sync?.status, sync?.findings.length ?? 0)) ??
    shown(fillLine(fillShown), toneOf(fill?.status, fill?.declined.length ?? 0)) ??
    // Then the runs that stopped and had the record taken from them, in the
    // refusal colour their own line is drawn in while they are the run on
    // record: nothing about them changed when the next folder started, and it
    // is the same sentence. Below the three above because those are about work
    // the server is on now or ended last; above the two below because these ran
    // and got part way, where a dropped folder is one nothing ever started on.
    shown(stoppedBooksLine(booksStopped), COLOR.refused) ??
    shown(stoppedLine(foldersStopped), COLOR.refused) ??
    // Last, because these are the only candidates about no run at all: the
    // folders a worker that ended without an answer threw away. They stand
    // where a line would stand once the run that died has had its say and been
    // put away, so that the news outlives the sentence it arrived beside. The
    // book queue leads, for the reason the freeze's own line leads the fill's.
    shown(droppedBooksLine(booksLost), COLOR.warn) ??
    shown(droppedLine(foldersLost), COLOR.warn);
  // What the line on the screen belongs to, so that the one dismiss button
  // beside it puts away the line a person is actually reading. The drop's own
  // line belongs to nothing that outlives it — it is over when the request is —
  // so it takes none.
  const dismissable = whoseLine(adding, freezeShown, syncShown, fillShown, {
    booksStopped,
    foldersStopped,
    booksLost,
    foldersLost,
  });
  // And the red line, with the button it answers named in it. It is read off
  // the same two runs the offers above are drawn from, so that the words it
  // names the press by are the words of a button standing on the bar.
  const refusal = refusalLine(trouble, fillShown, freezeShown);
  return (
    <footer
      style={{
        flex: '0 0 auto',
        display: 'flex',
        gap: 16,
        alignItems: 'center',
        padding: '5px 12px',
        background: COLOR.panel,
        borderTop: `1px solid ${COLOR.border}`,
        fontSize: 12,
        color: COLOR.dim,
      }}
    >
      {/* A refusal standing where the Library's name goes is not another dim
          line of housekeeping: it is the sentence saying why the screen above
          is empty, and it is coloured like the ones up there. */}
      <span style={library.status === 'failed' ? { color: COLOR.refused } : undefined}>
        {named(library)}
      </span>
      <button
        onClick={onLock}
        disabled={locking}
        title="Lock this Library on this device. Nothing can be read until the server is started again with the Passphrase."
        style={{ ...RETRY, cursor: locking ? 'default' : 'pointer' }}
      >
        {locking ? 'locking…' : 'lock'}
      </button>
      {/* The per-file line is what the candidates above fall through to, and not
          something shown beside them: it is the reader waiting on the page in
          front of it — the browser's own request lifecycle, which is all the
          server used to have a word for — while the fill is the folder being
          brought over behind it, and it is the larger thing happening. Both at
          once would be the bar reporting one fetch twice in two vocabularies. */}
      {line !== null ? (
        <span style={{ color: line.colour }}>{line.text}</span>
      ) : (
        fetching !== null && <span style={{ color: COLOR.text }}>fetching {fetching}…</span>
      )}
      {/* The way a person says they have read it. Offered from a run that is
          over and from nowhere else: a line about work happening now has counts
          still moving in it, and putting that away would last until the next
          answer arrived.

          It is what a finished sync's findings needed and never had — the one
          place somebody is told a dropped file is not backed up, standing in
          the bar for the life of the tab with every later fill's progress drawn
          underneath it. */}
      {dismissable !== null && (
        <button
          onClick={() => onDismiss(dismissable)}
          title={whatItBuys(dismissable)}
          style={RETRY}
        >
          dismiss
        </button>
      )}
      {/* The same offer for the sync, from the same state and for the same
          reason: a Storage that came back should not have to be met by adding a
          file that is already sitting in the folder.

          Each button says what it would do again rather than all of them saying
          "try again": one Storage outage stops the fill, the sync and the freeze
          together, and identical buttons side by side would leave a person
          guessing which of them the one line beside them belongs to. It stays
          an offer made from a stopped state and from nowhere else — nothing
          here is a "sync now". */}
      {retryable(syncShown) && (
        <button onClick={onRetrySync} style={RETRY}>
          back up again
        </button>
      )}
      {/* And the same for the book that was being packed. It is offered from the
          stopped state and from nowhere else, for the reason the other two are:
          nothing here is a "pack this" — what packs a book is bringing it in,
          and this is here so that a Storage that came back does not have to be
          met by dropping a book that is already sitting in the folder. */}
      {freezeShown !== null && retryable(freezeShown) && (
        <button onClick={() => onRetryFreeze(freezeShown.folder)} style={RETRY}>
          pack again
        </button>
      )}
      {/* Offered from the failed state and from nowhere else. It is not a
          download button: what brings a folder over is opening a file in it, and
          this is here so that a Storage that came back does not have to be met
          by opening a file that is already open. */}
      {fillShown !== null && retryable(fillShown) && (
        <button onClick={() => onRetryFill(fillShown.folder)} style={RETRY}>
          bring over again
        </button>
      )}
      {/* And one per run that stopped and then had the record taken from it.
          The same offer as the three above — a folder that got part way and is
          owed a second attempt — made by name because the run it is about is no
          longer the one on record, and a bare "bring over again" beside a line
          about the folder now running would name the wrong one.

          The same offer, so it is made on the same terms: a run whose
          explanation names the one recovery there is — a mapping only `coffret
          map` can settle — is offered no second attempt while it is the run on
          record, and being displaced is somebody else's folder starting rather
          than anything that happened to that failure. Its line stays, because
          the folder is half here either way; the button that would repeat what
          cannot change does not. */}
      {offeredAgain(foldersStopped).map((run) => (
        <button key={run.folder} onClick={() => onRetryFill(run.folder)} style={RETRY}>
          bring over {shownAs(run.folder)}
        </button>
      ))}
      {offeredAgain(booksStopped).map((run) => (
        <button key={run.folder} onClick={() => onRetryFreeze(run.folder)} style={RETRY}>
          pack {shownAs(run.folder)}
        </button>
      ))}
      {/* And one per folder the queue lost, which is a different offer from the
          one above it: that one takes up the folder that failed, and these take
          up the ones that never started. A single "try again" for both would
          leave a person pressing the folder they were told about and never
          reaching the ones they were not. */}
      {foldersLost.map((folder) => (
        <button key={folder} onClick={() => onRetryFill(folder)} style={RETRY}>
          bring over {shownAs(folder)}
        </button>
      ))}
      {/* And the same for the books a freeze's queue lost. A book that never
          started is a folder of pages sitting outside the Library with nothing
          on record about it, which is the state the retry exists for. */}
      {booksLost.map((folder) => (
        <button key={folder} onClick={() => onRetryFreeze(folder)} style={RETRY}>
          pack {shownAs(folder)}
        </button>
      ))}
      {/* What a press was refused with, standing after the buttons because it
          is about one of them. It opens with that button's own words: the
          buttons are side by side here and the sentence after them is the
          server's about the operation, so without the press named a person who
          pressed "bring over letters" and met "the Library is locked" cannot
          tell it from the same sentence about the book beside it. */}
      {refusal !== null && <span style={{ color: COLOR.refused }}>{refusal}</span>}
      {/* The control that asks what is new stands at the far end, apart from the
          three offers of a second attempt — those are made from a failure and go
          away with it, and this is always there.

          Its answer is said beside it rather than in the line above, because a
          refresh that is over has nothing to stop saying: in the line it would
          sit on top of the next fill's progress until something else happened. */}
      <span
        style={{ marginLeft: 'auto', display: 'flex', gap: 12, alignItems: 'center' }}
      >
        {refresh.refused !== null ? (
          <span style={{ color: COLOR.refused }}>{refresh.refused}</span>
        ) : (
          refresh.said !== null && <span>{refresh.said}</span>
        )}
        <button
          onClick={refresh.ask}
          disabled={refresh.running}
          style={{ ...RETRY, cursor: refresh.running ? 'default' : 'pointer' }}
        >
          {refresh.running ? 'looking…' : ASKING}
        </button>
      </span>
    </footer>
  );
}

/** The folders a fill's queue lost, which is none where nothing is on record. */
function droppedFolders(fill: Fill | null): readonly string[] {
  return fill?.dropped ?? [];
}

/** The books a freeze's queue lost, on the same terms. */
function droppedBooks(freeze: Freeze | null): readonly string[] {
  return freeze?.dropped ?? [];
}

/**
 * Which flow the line now on the bar belongs to, or `null` where the line
 * belongs to no run that can be put away.
 *
 * Read the same way the line itself is chosen, and it has to be: a dismiss
 * button that named a different run from the sentence beside it would put away
 * a line nobody is looking at and leave the one they are.
 */
function whoseLine(
  adding: string | null,
  freeze: Freeze | null,
  sync: Sync | null,
  fill: Fill | null,
  beside: {
    booksStopped: readonly Freeze[];
    foldersStopped: readonly Fill[];
    booksLost: readonly string[];
    foldersLost: readonly string[];
  },
): Dismissable | null {
  if (adding !== null) {
    return null;
  }
  if (freezeLine(freeze) !== null) {
    return canPutAway(freeze) ? { kind: 'line', flow: 'freeze' } : null;
  }
  if (syncLine(sync) !== null) {
    return canPutAway(sync) ? { kind: 'line', flow: 'sync' } : null;
  }
  if (fillLine(fill) !== null) {
    return canPutAway(fill) ? { kind: 'line', flow: 'fill' } : null;
  }
  // The four that are about no run the bar is drawing a line for. Every one of
  // them is over — a run that stopped is not going to be taken up again on its
  // own, and a lost folder never started — and without this they are the one
  // kind of notice a person cannot put away: the offer stands, with its
  // buttons, for as long as the server runs.
  //
  // By name and not by run, including for the two that are runs: see
  // [`Dismissable`](./dismissed).
  if (stoppedBooksLine(beside.booksStopped) !== null) {
    return { kind: 'stopped', queue: 'freeze', folders: folderNames(beside.booksStopped) };
  }
  if (stoppedLine(beside.foldersStopped) !== null) {
    return { kind: 'stopped', queue: 'fill', folders: folderNames(beside.foldersStopped) };
  }
  if (droppedBooksLine(beside.booksLost) !== null) {
    return { kind: 'dropped', queue: 'freeze', folders: beside.booksLost };
  }
  if (droppedLine(beside.foldersLost) !== null) {
    return { kind: 'dropped', queue: 'fill', folders: beside.foldersLost };
  }
  return null;
}

/** The folders a set of runs is about, which is what a dismissal of them holds. */
function folderNames(runs: readonly { folder: string }[]): readonly string[] {
  return runs.map((run) => run.folder);
}

/**
 * What one press of the dismiss button buys, said where it is offered.
 *
 * Three sentences and not one, because what brings each notice back is a
 * different event: the next run of that flow, a folder stopping again, a folder
 * being thrown away again. A person deciding whether to press it is deciding
 * how long the quiet lasts.
 */
function whatItBuys(dismissable: Dismissable): string {
  switch (dismissable.kind) {
    case 'line':
      return 'Put this away. It comes back for the next run.';
    case 'stopped':
      return 'Put this away. It comes back if a folder stops again.';
    case 'dropped':
      return 'Put this away. It comes back if a folder is thrown away again.';
  }
}

/**
 * The refusal as it goes on the bar: the words of the button it answers, then
 * what the server said.
 *
 * The prefix is the whole of it. One Storage outage stops the fill, the sync
 * and the freeze together, and every folder either queue lost is offered by its
 * own name beside them, so the bar can be holding out several buttons while it
 * has one line to refuse on — and the refusal is the server's sentence about
 * the operation, which names no button and often no folder either.
 *
 * Named by the button's words and not by the flow, because the words are what
 * the person is looking at: "bring over again" and "bring over letters" are two
 * different offers of the same flow, and telling somebody their *fill* was
 * refused tells them nothing about which of the two they pressed.
 */
function refusalLine(
  trouble: Trouble | null,
  fill: Fill | null,
  freeze: Freeze | null,
): string | null {
  return trouble === null ? null : `${pressedAs(trouble.pressed, fill, freeze)}: ${trouble.said}`;
}

/**
 * The words the button for that press is standing under.
 *
 * The two folder flows have two kinds of button and the press says only which
 * folder, so which kind it was is read back the way the buttons themselves are
 * drawn: a press of the folder a stopped run was on is that run's second
 * attempt, and any other folder is one the queue lost. Both do the same thing,
 * so on the rare occasion the bar offers both for one folder either phrase
 * names a button that would have done what was refused.
 *
 * Read back through [`offersAgain`](./retry), which is the same question the
 * refusal's own lifetime is decided by. Asked here in words of its own, the two
 * would drift: this one would go on calling a press "bring over again" after
 * the other had let its refusal go, and the line still standing would name a
 * button that is no longer on the bar. The runs handed in are the ones the bar
 * is drawing — a line somebody put away has taken its button with it, so its
 * words are not available to name a press by either.
 */
function pressedAs(pressed: Pressed, fill: Fill | null, freeze: Freeze | null): string {
  switch (pressed.flow) {
    case 'sync':
      return 'back up again';
    case 'fill':
      return offersAgain(fill, pressed.folder)
        ? 'bring over again'
        : `bring over ${shownAs(pressed.folder)}`;
    case 'freeze':
      return offersAgain(freeze, pressed.folder) ? 'pack again' : `pack ${shownAs(pressed.folder)}`;
  }
}

/** One line and the colour it is drawn in, where there is a line to draw. */
function shown(text: string | null, colour: string): { text: string; colour: string } | null {
  return text === null ? null : { text, colour };
}

/**
 * What a line about one of the three runs is drawn in.
 *
 * A run that stopped is a refusal, in the colour every refusal on this screen is
 * in. A run that finished and found something is in the warn colour, for the
 * reason the rows use it: something to be told rather than shown, and not the
 * failure of anything anybody asked for. Drawn as ordinary text it would read as
 * the housekeeping beside it and be looked past, which for the one sentence
 * saying a dropped file is not backed up — or the one saying a file this device
 * asked for was not placed — is the whole loss. What a fill found is the Entries
 * it declined, which is the same kind of news about the same kind of file.
 *
 * A freeze that finished and found nothing still has a line — it says what the
 * book came to — and that one is ordinary text: it is good news, and the warn
 * colour on it would make a book that packed perfectly look like a problem.
 */
function toneOf(status: string | undefined, findings: number): string {
  switch (status) {
    case 'stopped':
      return COLOR.refused;
    case 'done':
      return findings === 0 ? COLOR.text : COLOR.warn;
    default:
      return COLOR.text;
  }
}

/**
 * What every button along this bar is drawn as: the three offers of a second
 * attempt, and the one that asks the Library what is new.
 */
const RETRY = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '1px 10px',
  borderRadius: 4,
  cursor: 'pointer',
} as const;

/** The Library root has no name of its own, and is not called the empty string. */
function shownAs(folder: string): string {
  return folder === '' ? 'the Library root' : folder;
}

function named(library: Remote<Library>): string {
  switch (library.status) {
    case 'loading':
      return 'opening the Library…';
    case 'ready':
      return `${library.value.name} — on ${library.value.provider}`;
    case 'failed':
      return library.message;
  }
}
