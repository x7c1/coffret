import findingReasons from './finding-reasons.json';
import type { PlacementReason, Refused, SurfacedFinding } from './refusal';
import { refusedOf, surfacedOf } from './refusal';
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

/** One Entry a fill did not bring over, and what it found instead. */
export interface DeclinedEntry extends Refused {
  path: string;
}

/**
 * A run that stopped, and what stopped it.
 *
 * The two fields travel together or not at all: a run whose status says it
 * stopped always says what stopped it, and a run that did not stop carries no
 * refusal. So a screen reading the sentence off a stopped run has one to read,
 * and never has to make one up.
 */
export interface Stopped {
  status: 'stopped';
  /** What stopped the run. */
  stopped: Refused;
}

/** A run in any state but stopped, which carries no refusal. */
export interface NotStopped<Status extends string> {
  status: Status;
  stopped: null;
}

/** A run's status and its refusal, in the only pairs they come in. */
type Standing<Status extends string> = NotStopped<Exclude<Status, 'stopped'>> | Stopped;

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
  | 'settling'
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
 * A count of things done out of things to do: what a person watching a
 * transfer wants to know is whether it is moving and roughly how much is left.
 * The upload also counts bytes, because its unit can be a Pack of tens of
 * megabytes, and a book that is one Pack reads `0/1` for as long as it takes to
 * send.
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
  /**
   * How many bytes of the phase Storage has taken, out of how many it sends,
   * and `null` for a phase that does not count them — every phase but
   * `uploading`.
   *
   * Of the whole phase rather than of the unit in flight, so it reads beside
   * `done` without saying which unit it is about. It can go back where a put
   * that failed is tried again, since that sends its object from the start.
   */
  bytes: ByteCount | null;
}

/** How many bytes of a phase have gone, out of how many it sends. */
export interface ByteCount {
  done: number;
  total: number;
}

/** What one fill came to, whether it is on record or was displaced. */
interface FillOfItsOwn {
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
   * What it found and did not act on: what its reads found of the Library's
   * committed Keyring, said once however many Entries met it. None of it stops
   * the run, and none of it is about one row — the files still open.
   */
  findings: Finding[];
}

/** The fill's queue, which is the flow's rather than any one run's. */
interface FillQueue {
  /**
   * The folders asked for by name that are waiting their turn behind this one,
   * oldest first.
   *
   * The names and not a count, for the reason a freeze's queue carries names: a
   * person who pressed a button wants to know that theirs is coming. A folder
   * asked for by name waits its turn rather than superseding the fill in
   * progress, and between the press and the run this is the only thing said
   * about it — the line names the folder being brought over, and the button
   * that named this one goes away as the queue takes it.
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
  discarded: string[];
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
   * `discarded` and this list belong to the flow rather than to any run, so they
   * are reported once, on the run the flow is on, and a displaced run does not
   * carry them.
   *
   * The newest eight at most. Clicking from folder to folder while Storage is
   * down stops a fill per click, and past that many the oldest is forgotten —
   * its line, not its files: the folder's rows still say `remote`, and opening
   * one of them brings the folder over as it always did.
   */
  displaced: DisplacedFill[];
}

/**
 * What the server is bringing over on its own.
 *
 * Opening a file this device does not have fetches it and then goes on to fetch
 * the rest of its folder, unasked — nobody who opened page one stops there.
 * This is that work's account of itself, and it is the server's own state: it
 * says nothing about what the Library holds, it is gone when the server is, and
 * `present` and `remote` stay the listing's to say.
 *
 * `stopped` says what stopped the fill exactly where `status` is `stopped`, and
 * is `null` everywhere else.
 */
export type Fill = FillOfItsOwn & FillQueue & Standing<FillStatus>;

/**
 * A fill that stopped and that a later one took the record from.
 *
 * Narrower than a {@link Fill}: it stopped, so it always says what stopped it,
 * and it carries none of the queue's lists, which are the run on record's to
 * report. What it keeps is what its line and its rows are drawn from — the
 * counts, and the Entries it declined.
 */
export type DisplacedFill = FillOfItsOwn & Stopped;

/** Where a sync stands. */
export type SyncStatus =
  /** Armed, or walking the mapped folders. */
  | 'syncing'
  /** It finished, whatever it found. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped';

/**
 * Which way a run left something alone, in a refusal's own words.
 *
 * The same vocabulary as {@link PlacementReason}, spelled the same way, because
 * the states are the same ones: one Entry whose Container the Library records
 * no key for is `locked` whether a fetch declined it or a run reported it, and
 * a mapped folder that is not the one its mapping was recorded against is
 * `refused_root` either way. Those three are taken from `PlacementReason`
 * itself, so a spelling changed there drops out of this union and the literals
 * written for it stop compiling; the four a refusal never carries are a run's
 * own. The list the server sends is `finding-reasons.json`: the server's tests
 * hold what it sends to that file, and `work.test.ts` holds this union to it.
 */
export type FindingReason =
  /** A finding about one Entry; `surfaced` says which. */
  | Extract<PlacementReason, 'surfaced'>
  /**
   * One Entry whose Container the Library records no key for (with `surfaced`
   * `KeyLost`), or a Container the run met that it has no key for (without).
   */
  | Extract<PlacementReason, 'locked'>
  /** A folder this device maps is not the folder its mapping was recorded against. */
  | Extract<PlacementReason, 'refused_root'>
  /** A folder this device maps is not there, so nothing in it was looked at. */
  | 'root_missing'
  /**
   * A folder this device maps is empty and stands on another filesystem, so
   * nothing in it was looked at.
   */
  | 'root_on_another_filesystem'
  /**
   * The Library's committed Keyring, read through with some of its replicas
   * missing, unreadable or not handed over; the message says whether a loss
   * was established. Files still open, and the next run that writes repairs
   * it, so nothing waits on the person reading it.
   */
  | 'keyring_degraded'
  /**
   * The Library's committed Keyring was short and the run put it back: the
   * message names the generation and how many replicas were rewritten, in the
   * words the command line says it in. Carried by a run that finished and by
   * one that stopped after its commit repaired the set; the set is whole
   * again, so nothing waits on the person reading it.
   */
  | 'keyring_repaired';

/**
 * One thing a run that succeeded still has to say — a finding, in the word the
 * Library's own vocabulary gives it.
 *
 * Not a refusal: nothing was refused, the run succeeded, and this is what it
 * left alone — a file whose Entry lives in a Pack, a file this device no longer
 * has, a mapped root it could not vouch for. Reading only the counts would tell
 * somebody their file is backed up when it is not. A sync and a freeze report
 * them in the one shape. The one kind a run that stopped carries too is a
 * Keyring repair its commit performed before it failed, because the replicas it
 * put back stand whatever became of the run (spec: KL-15).
 *
 * It names what it is about in the two fields a declined fetch does, paired
 * the same way — `locked` beside `KeyLost`, `surfaced` beside every other
 * name — so a page reads one with the branches it already has for the other.
 */
export interface Finding {
  /** The Entry this is about, and `null` where it is about no single one. */
  path: string | null;
  /** The sentence to show beside the row. */
  message: string;
  /**
   * Which way the run left it alone, and `null` for a reason this client has not
   * heard of — as a refusal's reason is.
   */
  reason: FindingReason | null;
  /**
   * The finding about one Entry, by the name the device layer gives it, and
   * `null` for one about a mapping or a Container, or for a name this client has
   * not heard of.
   */
  surfaced: SurfacedFinding | null;
}

/**
 * What the server is carrying into the Library on its own.
 *
 * Dropping a file means "add this", and adding is not finished when the bytes
 * reach the folder: the server runs the same sync the person would have typed,
 * and this is that run's account of itself. Like a fill it is the server's own
 * state — gone when the server is, and never uploaded.
 */
export type Sync = SyncOfItsOwn & Standing<SyncStatus>;

/** What one sync came to, beside its status. */
interface SyncOfItsOwn {
  /** Which run of the sync this is, counted from the start of the server. */
  run: number;
  /** How many files the run carried in, and `0` until it is over. */
  added: number;
  /**
   * What it found and did not act on, and the Keyring repairs its commit
   * performed — on a run that stopped, those repairs and nothing else.
   */
  findings: Finding[];
  /**
   * How far into the walk the flow says it has got, and `null` before it has
   * said and once the run is over.
   */
  step: Step | null;
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
export type Freeze = FreezeOfItsOwn & FreezeQueue & Standing<FreezeStatus>;

/**
 * A freeze that stopped and that a later one took the record from.
 *
 * Narrower than a {@link Freeze} on the terms a {@link DisplacedFill} is: it
 * always says what stopped it, and carries none of the queue's lists. What it
 * keeps is what its line is drawn from.
 */
export type DisplacedFreeze = FreezeOfItsOwn & Stopped;

/** What one freeze came to, whether it is on record or was displaced. */
interface FreezeOfItsOwn {
  /** Which run of the freeze this is, counted from the start of the server. */
  run: number;
  /** The folder being packed; the Library root is the empty string. */
  folder: string;
  /** How many Packs the run built, and `0` until it is over. */
  packs: number;
  /** How many Entries those Packs hold, and `0` until it is over. */
  entries: number;
  /**
   * What it found and did not act on, and the Keyring repairs its commit
   * performed — on a run that stopped, those repairs and nothing else.
   */
  findings: Finding[];
  /**
   * How far into the run the flow says it has got, and `null` before it has
   * said and once the run is over.
   */
  step: Step | null;
}

/** The freeze's queue, which is the flow's rather than any one run's. */
interface FreezeQueue {
  /**
   * The books waiting their turn behind this one, oldest first.
   *
   * The names and not a count: a person who dropped a second book wants to know
   * that theirs is queued, and a bare number says only that somebody's is.
   */
  waiting: string[];
  /** The books thrown away when the work ended without an answer. */
  discarded: string[];
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
  displaced: DisplacedFreeze[];
}

/**
 * Which of the two states this device holds the Library in.
 *
 * The two words the spec calls them by, and the only two there are: the
 * Passphrase moves a device from the first to the second — given in the desktop
 * app's own window for a server running inside it, or by starting a
 * command-line server again — and a lock — the interval the server went
 * unasked for — moves it back.
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
  /** The last one did not finish, and `stopped` says what stopped it. */
  | 'behind';

/**
 * How the catalog stands, and what stopped it where something did.
 *
 * `stopped` says what stopped the last catch-up exactly where the catalog is
 * `behind`, and is `null` everywhere else.
 */
export type Catalog =
  | { state: Exclude<CatalogState, 'behind'>; stopped: null }
  | { state: 'behind'; stopped: Refused };

/**
 * Where a reconnect stands: the consent flow `POST /api/reconnect` starts to
 * renew the grant Storage stopped taking.
 */
export type ReconnectState =
  /** A consent page is open, and the server is waiting for it to be answered. */
  | 'waiting'
  /** The grant was renewed, and the catalog was asked to catch up with it. */
  | 'renewed'
  /** The person declined on the consent page. */
  | 'refused'
  /** Nobody answered the consent page before the server stopped waiting. */
  | 'timed_out'
  /** The flow ended some other way, which the server's log says. */
  | 'failed';

/** The last reconnect, and the server's sentence about it. */
export interface Reconnect {
  state: ReconnectState;
  /** The server's own sentence, written to be read by a person. */
  message: string;
}

/** What the server is doing on its own — `GET /api/work`. */
export interface Work {
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
   * It matters because a restart is ordinary rather than exceptional: a server
   * started from the command line is unlocked by starting it again with the
   * Passphrase, and a tab left open across one would otherwise hide the new
   * process's first runs — a fill's line and its declined Entries, and the one
   * sentence saying a sync did not back a file up. An unlock in place, from the
   * desktop app's own window, keeps the name: it is the same process.
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
  /**
   * The latest reconnect, waiting or ended, and `null` where none has run.
   *
   * A consent flow ends at a person's browser, on nobody's clock, and this is
   * where the page that opened the consent page learns how it ended. It never
   * carries the page itself: that goes back only to the press that asked.
   */
  reconnect: Reconnect | null;
}

/** Asks what the server is doing on its own. */
export async function getWork(signal?: AbortSignal): Promise<Work> {
  return workOf(await askedForJson<unknown>(apiUrl('work'), signal));
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
export async function startSync(signal?: AbortSignal): Promise<Work> {
  return workOf(await askedForJson<unknown>(apiUrl('sync'), signal, 'POST'));
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
 * What it asks for waits its turn rather than superseding the fill in progress,
 * unlike the fetch that arms one implicitly: every folder named here was named
 * by a button somebody pressed, so two presses bring both folders over, in the
 * order they were pressed.
 *
 * It answers with the work answer as it stands the moment the fill is armed rather
 * than waiting for the work, which is why the caller goes on polling.
 */
export async function startFill(folder: string, signal?: AbortSignal): Promise<Work> {
  return workOf(
    await askedForJson<unknown>(
      apiUrl('fill', folder === '' ? undefined : { path: folder }),
      signal,
      'POST',
    ),
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
 * It answers with the work answer as it stands the moment the freeze is armed
 * rather than waiting for the work, which is why the caller goes on polling.
 */
export async function startFreeze(folder: string, signal?: AbortSignal): Promise<Work> {
  return workOf(
    await askedForJson<unknown>(
      apiUrl('freeze', folder === '' ? undefined : { path: folder }),
      signal,
      'POST',
    ),
  );
}

/**
 * The work answer as the server sent it, before its refusals and findings are
 * read.
 *
 * The rest of the answer is taken as the server's serialization, the way every
 * other answer of this package is. What is not taken on trust is the vocabulary
 * inside it: the refusals and the findings name kinds, reasons and findings out
 * of unions a screen branches on, and those go through the one narrowing a
 * refused request goes through.
 */
interface WorkSent {
  server: string;
  library: LibraryState;
  catalog: { state: CatalogState; stopped: unknown };
  fill: FillSent | null;
  sync: SyncSent | null;
  freeze: FreezeSent | null;
  reconnect: Reconnect | null;
}

/** A run as sent: its refusal and its findings not read yet. */
type Sent<Run> = Omit<Run, 'status' | 'stopped' | 'declined' | 'findings' | 'displaced'> & {
  status: string;
  stopped: unknown;
  declined?: unknown[];
  findings?: unknown[];
  displaced?: unknown[];
};
type FillSent = Sent<FillOfItsOwn & FillQueue>;
type SyncSent = Sent<SyncOfItsOwn>;
type FreezeSent = Sent<FreezeOfItsOwn & FreezeQueue>;

/**
 * One work answer, read.
 *
 * Every refusal in it — what stopped each run, what stopped the catalog, each
 * Entry a fill declined, and the same of every displaced run — is read by
 * {@link refusedOf}, so a kind this client has not heard of is `unrecognized`
 * and a reason or a finding name it has not heard of is `null`, exactly where a
 * refused request would put them. Each finding's reason and name are read the
 * same way.
 *
 * It never throws over a value it does not know. A status and its refusal are
 * paired as the types pair them: the server sends a refusal exactly where a run
 * stopped, and this reads the refusal off that pairing rather than off whether
 * the field happened to be there.
 */
export function workOf(sent: unknown): Work {
  const work = sent as WorkSent;
  return {
    server: work.server,
    library: work.library,
    catalog: catalogOf(work.catalog),
    fill: work.fill === null ? null : fillOf(work.fill),
    sync: work.sync === null ? null : syncOf(work.sync),
    freeze: work.freeze === null ? null : freezeOf(work.freeze),
    reconnect: work.reconnect,
  };
}

function catalogOf(catalog: WorkSent['catalog']): Catalog {
  return catalog.state === 'behind'
    ? { state: 'behind', stopped: refusedOf(catalog.stopped) }
    : { state: catalog.state, stopped: null };
}

/** A status and its refusal, in the pair the types hold them to. */
function standingOf<Status extends string>(run: {
  status: string;
  stopped: unknown;
}): Standing<Status> {
  return run.status === 'stopped'
    ? { status: 'stopped', stopped: refusedOf(run.stopped) }
    : { status: run.status as Exclude<Status, 'stopped'>, stopped: null };
}

function fillOf(fill: FillSent): Fill {
  return {
    ...fillOfItsOwn(fill),
    waiting: fill.waiting,
    discarded: fill.discarded,
    displaced: (fill.displaced ?? []).map((run) => displacedFillOf(run as FillSent)),
    ...standingOf<FillStatus>(fill),
  };
}

function displacedFillOf(run: FillSent): DisplacedFill {
  return { ...fillOfItsOwn(run), status: 'stopped', stopped: refusedOf(run.stopped) };
}

function fillOfItsOwn(run: FillSent): FillOfItsOwn {
  return {
    run: run.run,
    folder: run.folder,
    total: run.total,
    done: run.done,
    declined: (run.declined ?? []).map(declinedOf),
    findings: (run.findings ?? []).map(findingOf),
  };
}

function declinedOf(entry: unknown): DeclinedEntry {
  return { path: (entry as { path: string }).path, ...refusedOf(entry) };
}

function syncOf(sync: SyncSent): Sync {
  return {
    run: sync.run,
    added: sync.added,
    findings: (sync.findings ?? []).map(findingOf),
    step: sync.step,
    ...standingOf<SyncStatus>(sync),
  };
}

function freezeOf(freeze: FreezeSent): Freeze {
  return {
    ...freezeOfItsOwn(freeze),
    waiting: freeze.waiting,
    discarded: freeze.discarded,
    displaced: (freeze.displaced ?? []).map((run) => displacedFreezeOf(run as FreezeSent)),
    ...standingOf<FreezeStatus>(freeze),
  };
}

function displacedFreezeOf(run: FreezeSent): DisplacedFreeze {
  return { ...freezeOfItsOwn(run), status: 'stopped', stopped: refusedOf(run.stopped) };
}

function freezeOfItsOwn(run: FreezeSent): FreezeOfItsOwn {
  return {
    run: run.run,
    folder: run.folder,
    packs: run.packs,
    entries: run.entries,
    findings: (run.findings ?? []).map(findingOf),
    step: run.step,
  };
}

/**
 * The finding reasons the server can send, read from the file its cases hold
 * to what it builds — the list {@link FindingReason} is held to as well.
 */
const FINDING_REASONS: readonly string[] = findingReasons;

/**
 * One finding, read: its reason and its name narrowed as a refusal's are, so
 * one this client has not heard of is `null` rather than a string claiming a
 * union.
 */
function findingOf(sent: unknown): Finding {
  const finding = sent as {
    path: string | null;
    message: string;
    reason?: unknown;
    surfaced?: unknown;
  };
  return {
    path: finding.path,
    message: finding.message,
    reason:
      typeof finding.reason === 'string' && FINDING_REASONS.includes(finding.reason)
        ? (finding.reason as FindingReason)
        : null,
    surfaced: surfacedOf(finding.surfaced),
  };
}
