import type { DeclinedReason, RefusalKind, SurfacedFinding } from './refusal';
import { apiUrl, askedForJson } from './request';

/** Where a fill of one folder stands. */
export type FillStatus =
  /** Armed, or walking the folder. */
  | 'filling'
  /** Every file it set out to bring over is here or accounted for. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped'
  /** A fetch landed in another folder and the fill followed it there. */
  | 'superseded';

/**
 * One refusal, in the shape every refusal from this server takes.
 *
 * The same fields a refused request carries, so a declined Entry is read with
 * the same three branches: which kind, which way, which finding.
 */
export interface Refused {
  error: RefusalKind;
  message: string;
  reason?: DeclinedReason;
  surfaced?: SurfacedFinding;
}

/** One Entry a fill did not bring over, and what it found instead. */
export interface DeclinedEntry extends Refused {
  path: string;
}

/**
 * Which phase of a flow a run is in.
 *
 * The whole set is named here for the reason every other union in this package
 * is: a caller writes a branch per phase, and one it has never heard of is one
 * it falls off the end of.
 *
 * They are the use case layer's own phases rather than this client's reading of
 * them — the same ones the command line draws its progress line from — so a
 * browser and a terminal watching one run cannot call it two different things.
 */
export type Phase =
  /** Bringing this device's catalog up to the Library's head. */
  | 'catching_up'
  /** Settling the pending rows an interrupted run left behind. */
  | 'reconciling'
  /** Reading this device's mapped folders and deciding what the run will do. */
  | 'scanning'
  /** Encoding local files into Containers on this device. */
  | 'packing'
  /** Sending encoded Containers to Storage. */
  | 'uploading'
  /** Reading Containers back from Storage and placing the files in them. */
  | 'fetching';

/**
 * How far into one phase a run has got.
 *
 * A count of things done out of things to do, and never a byte count: what a
 * person watching a transfer wants to know is whether it is moving and roughly
 * how much is left.
 */
export interface Step {
  phase: Phase;
  /** How many units of that phase have finished. */
  done: number;
  /**
   * How many there are in all, and `null` where the phase cannot say.
   *
   * Not the same state as `0`, and they must not be shown alike: a phase that
   * cannot count its work is exactly the phase that goes quiet for minutes, and
   * what a screen shows for it is the phase's name without numbers.
   */
  total: number | null;
}

/**
 * What the server is bringing over on its own.
 *
 * Opening a file this device does not have fetches it and then goes on to fetch
 * the rest of its folder, unasked — nobody who opened page one stops there.
 * This is that work's account of itself, and it is the server's own state: it
 * says nothing about what the Library holds, it is gone when the server is, and
 * `present` and `remote` stay the listing's to say.
 */
export interface Fill {
  /**
   * Which run of the fill this is, counted from the start of the server.
   *
   * What tells one run's account of itself from the next's. A screen that lets
   * somebody put a line away remembers which run they put away by this, so that
   * doing so does not silently take the next run's line with it.
   */
  run: number;
  /** The folder being brought over; the Library root is the empty string. */
  folder: string;
  status: FillStatus;
  /**
   * How many of the folder's files the fill set out to bring over, and `0`
   * until it has read the folder's listing.
   */
  total: number;
  /** How many of them are on this device now. */
  done: number;
  /** The Entries it declined, each with what opening it would have said. */
  declined: DeclinedEntry[];
  /**
   * The folders asked for by name that are waiting their turn behind this one,
   * oldest first.
   *
   * The names and not a count, for the reason a freeze's queue carries names: a
   * person who pressed a button wants to know that theirs is coming. A folder
   * asked for by name waits rather than displacing the fill in progress, and
   * between the press and the run this is the only thing said about it — the
   * line names the folder being brought over, and the button that named this
   * one goes away as the queue takes it.
   */
  waiting: string[];
  /**
   * The folders that were queued behind a fill and thrown away when the work
   * ended without an answer, and that nobody has taken up since.
   *
   * Not this fill's own doing and not gone when it is: the line and the retry
   * beside it name the folder that died, and these are the ones nothing on the
   * screen would otherwise mention at all. Asking for one again is what takes
   * it off this list.
   */
  dropped: string[];
  /**
   * The runs that stopped and that a later one took the record from, oldest
   * first.
   *
   * Not the list above: those folders were thrown away before anything started
   * on them, and these ran and got part way. What the server holds for a browser
   * is one run at a time, and the next folder off the queue takes the record
   * from a stopped one within a tick — somebody clicking into another folder
   * while Storage is down is enough — so without these the line naming that
   * folder, the Entries it declined and the offer of a second attempt would all
   * go unread.
   *
   * Each says what its own run came to and nothing about the queue: `waiting`,
   * `dropped` and this list belong to the flow rather than to any run, so they
   * are reported once, on the run the flow is on, and arrive empty here.
   */
  displaced: Fill[];
  /** What stopped the fill, and `null` where nothing did. */
  stopped: Refused | null;
}

/** Where a sync stands. */
export type SyncStatus =
  /** Armed, or walking the mapped folders. */
  | 'syncing'
  /** It finished, whatever it found. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped';

/**
 * One thing a sync that succeeded still has to say.
 *
 * Not a refusal: nothing was refused, the run succeeded, and this is what it
 * left alone — a file whose Entry lives in a Pack, a file this device no longer
 * has, a mapped root it could not vouch for. Reading only the counts would tell
 * somebody their file is backed up when it is not.
 */
export interface SyncFinding {
  /** The Entry this is about, and `null` where it is about no single one. */
  path: string | null;
  message: string;
}

/**
 * What the server is carrying into the Library on its own.
 *
 * Dropping a file means "add this", and adding is not finished when the bytes
 * reach the folder: the server runs the same sync the person would have typed,
 * and this is that run's account of itself. Like a fill it is the server's own
 * state — gone when the server is, and never uploaded.
 */
export interface Sync {
  /** Which run of the sync this is, counted from the start of the server. */
  run: number;
  status: SyncStatus;
  /** How many files the run carried in, and `0` until it is over. */
  added: number;
  /** What it found and did not act on. */
  noted: SyncFinding[];
  /**
   * How far into the walk the flow says it has got, and `null` before it has
   * said and once the run is over.
   */
  step: Step | null;
  /** What stopped the sync, and `null` where nothing did. */
  stopped: Refused | null;
}

/** Where a freeze of one folder stands. */
export type FreezeStatus =
  /** Armed, or packing the folder. */
  | 'freezing'
  /** It finished, whatever it found. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped';

/**
 * What the server is packing into the Library on its own.
 *
 * Dropping a book into a folder made for it means "bring this in", and a book is
 * the one thing a sync is the wrong shape for: a folder of a few hundred page
 * images would become a few hundred Storage objects, a few hundred uploads, and
 * a few hundred calls to open it again. So the server packs them instead, and
 * this is that run's account of itself. Like a fill and a sync it is the
 * server's own state — gone when the server is, and never uploaded.
 *
 * The counts are outcomes and stay `0` until the batch commits: a freeze builds
 * and commits one batch, so until it has committed no number of Packs would be
 * true. Where the run has got to is a different question and `step` answers it.
 */
export interface Freeze {
  /** Which run of the freeze this is, counted from the start of the server. */
  run: number;
  /** The folder being packed; the Library root is the empty string. */
  folder: string;
  status: FreezeStatus;
  /** How many Packs the run built, and `0` until it is over. */
  packs: number;
  /** How many Entries those Packs hold, and `0` until it is over. */
  entries: number;
  /** What it found and did not act on. */
  noted: SyncFinding[];
  /**
   * How far into the run the flow says it has got, and `null` before it has
   * said and once the run is over.
   */
  step: Step | null;
  /**
   * The books waiting their turn behind this one, oldest first.
   *
   * The names and not a count: a person who dropped a second book wants to know
   * that theirs is queued, and a bare number says only that somebody's is.
   */
  waiting: string[];
  /** The books thrown away when the work ended without an answer. */
  dropped: string[];
  /**
   * The runs that stopped and that a later one took the record from, oldest
   * first.
   *
   * Not the list above: those books were thrown away before anything started on
   * them, and these ran and got part way. A freeze Storage stopped leaves its
   * pages on the disk and out of the Library, in a folder made in this browser
   * that the Library has never heard of — so this run is the only thing naming
   * that place, and a second book queued behind the first is all it takes for
   * the next run to take the record from it. Without these a reload would draw
   * no row for the folder, offer no way to walk in, and make no second attempt
   * at it.
   *
   * Each says what its own run came to and nothing about the queue, on the terms
   * a fill's do.
   */
  displaced: Freeze[];
  /** What stopped the freeze, and `null` where nothing did. */
  stopped: Refused | null;
}

/**
 * Which of the two states this device holds the Library in.
 *
 * The two words the spec calls them by, and the only two there are: the
 * Passphrase moves a device from the first to the second, and a lock — one
 * somebody asked for, or the interval the server went unasked for — moves it
 * back.
 */
export type LibraryState = 'locked' | 'unlocked';

/**
 * How far this device's catalog has got with the Library.
 *
 * Every listing comes out of the catalog, and the catalog holds what this
 * device has replayed — so an explorer showing nothing is showing either a
 * Library with nothing in it or a device that has not learnt what is in it, and
 * from the rows alone the two are the same screen.
 */
export type CatalogState =
  /** A catch-up is running; what is listed is what was known before it began. */
  | 'catching_up'
  /** The last one finished, so this device has replayed what the Library had. */
  | 'caught_up'
  /** The last one did not finish, and `trouble` says what stopped it. */
  | 'behind';

/** How the catalog stands, and what stopped it where something did. */
export interface Catalog {
  state: CatalogState;
  /** What stopped the last catch-up, and `null` where nothing did. */
  trouble: Refused | null;
}

/** What the server is doing on its own — `GET /api/activity`. */
export interface Activity {
  /**
   * What the process that answered calls itself.
   *
   * The same string for every answer one process gives, and a different one
   * after it is started again. It says nothing about the device or the
   * Library — the server composes it out of entropy as it starts — and what it
   * is for is everything a page remembers between answers that is true of one
   * process only: the run numbers below all start again at 1 with the next one,
   * so a name the page has not seen before is the sign to forget what it was
   * holding. The run numbers cannot say it themselves, since one lower than a
   * run already put away is equally what an answer issued just before the
   * dismissal looks like.
   *
   * It matters because a restart is ordinary rather than exceptional: a locked
   * Library is unlocked by typing the Passphrase and starting the server again,
   * and a tab left open across one would otherwise hide the new process's first
   * runs — a fill's line and its declined Entries, and the one sentence saying
   * a sync did not back a file up.
   */
  server: string;
  /**
   * Whether this device still holds the Library open.
   *
   * Read from this route and from no other, because this is the one a screen
   * asks without being told to. A lock that happened on the server's own clock
   * tells nobody, so a window left open over a page it decrypted learns of it
   * here or not until its next request is refused.
   */
  library: LibraryState;
  /**
   * How this device's catalog stands with the Library.
   *
   * Here for the reason the field above it is: it is what the server did to
   * itself rather than work anybody asked for, and this is the one question a
   * page asks without being told to — which is exactly when an empty Library
   * has to be told apart from a Library nobody has looked at yet.
   */
  catalog: Catalog;
  /** The latest fill, running or finished, and `null` where none has run. */
  fill: Fill | null;
  /** The latest sync, running or finished, and `null` where none has run. */
  sync: Sync | null;
  /** The latest freeze, running or finished, and `null` where none has run. */
  freeze: Freeze | null;
}

/** Asks what the server is doing on its own. */
export function getActivity(signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(apiUrl('activity'), signal);
}

/**
 * Carries the mapped folders into the Library again — `POST /api/sync`.
 *
 * Not a "sync now" button and not offered as one. What syncs a dropped file is
 * dropping it; this exists for the state that leaves behind — a sync Storage
 * stopped, whose files are sitting in the folder with nothing left to drop —
 * where the alternative is telling somebody to add a file they have added.
 *
 * It takes no folder. Which folders a sync walks is the device's mappings and
 * never an argument, here as on the command line.
 */
export function startSync(signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(apiUrl('sync'), signal, 'POST');
}

/**
 * Takes one folder up again — `POST /api/fill?path=`.
 *
 * Not a download button and not offered as one. What brings a folder over is
 * opening a file in it; this exists for the three states that leaves behind — a
 * fill Storage stopped, one superseded when somebody clicked elsewhere, and a
 * folder a worker that died threw away before it began — where the alternative
 * is telling a person to open a file they have opened.
 *
 * What it asks for waits its turn rather than displacing the fill in progress,
 * unlike the fetch that arms one implicitly: every folder named here was named
 * by a button somebody pressed, so two presses bring both folders over, in the
 * order they were pressed.
 *
 * It answers with the activity as it stands the moment the fill is armed rather
 * than waiting for the work, which is why the caller goes on polling.
 */
export function startFill(folder: string, signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(
    apiUrl('fill', folder === '' ? undefined : { path: folder }),
    signal,
    'POST',
  );
}

/**
 * Packs one folder into Packs again — `POST /api/freeze?path=`.
 *
 * Not a "pack this" button and not offered as one. What packs a book is bringing
 * it in — dropping its pages onto a folder made a moment ago, which arms this
 * itself — and this exists for the state that leaves behind: a freeze Storage
 * stopped, whose pages are sitting in the folder with nothing left to drop,
 * where the alternative is telling somebody to drop a book they have dropped.
 *
 * It takes a folder, unlike the sync: a freeze is of one folder, and one
 * narrowed to nothing would pack the whole Library.
 *
 * It answers with the activity as it stands the moment the freeze is armed
 * rather than waiting for the work, which is why the caller goes on polling.
 */
export function startFreeze(folder: string, signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(
    apiUrl('freeze', folder === '' ? undefined : { path: folder }),
    signal,
    'POST',
  );
}
