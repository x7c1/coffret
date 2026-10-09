import surfacedFindings from './surfaced-findings.json';

/**
 * Which kind of refusal an answer is.
 *
 * The first thirteen are the server's own, and the whole set is named here for the
 * reason the server names it: a caller writes a branch per kind, and a kind it
 * has never heard of is one it falls off the end of. Adding one on the server
 * is adding a case here.
 *
 * The last two are this client's, minted where no answer of the server's shape
 * arrived at all. They are kept in the same union so that everything a caller
 * has to handle is one type: a page that has to say why it is empty does not
 * care whether the sentence came from the server or from here.
 */
export type RefusalKind =
  | 'bad_path'
  /** The request itself was not one the route could read. */
  | 'bad_request'
  /**
   * The server does not answer this caller: the request showed no key of its,
   * arrived by a name that is not the server's own, or came from a page on
   * another site. It is refused before any route sees it, so it says nothing
   * about the Library.
   */
  | 'unauthorized'
  | 'no_such_entry'
  /**
   * The server answers nothing at the path that was asked, or answers that path
   * by other methods than the one it was asked by — `404` and `405`, one kind
   * between them. It is this server replying, which is why it is not
   * `unrecognized`: a page of an older build asking for a route since renamed
   * meets this, and the sentence names neither the path nor the method.
   */
  | 'no_such_route'
  /**
   * A fetch did not place one Entry, and the reason says why: the Entry Path
   * concept's *decline*, a verdict on that one Entry. A drop declines a file
   * the same way where its own path cannot be placed (`unmaterializable`,
   * `reserved`), reporting it beside what it placed.
   */
  | 'declined'
  /**
   * A placement this device will not make, whether of the one file at a path
   * or of every file under a mapped root: the concept's *refuse*, which is
   * wider than a decline. A mapping's root that is not the folder it was
   * recorded against (`refused_root`), a drop or a freeze under a folder this
   * device has no folder for (`unmapped`), and a drop that would replace an
   * Entry inside a Pack (`pack_resident`).
   */
  | 'refused_placement'
  /**
   * This device has to be enrolled in the Library again: a Master Key epoch was
   * activated, and the device holds only the key it replaced. Nothing a retry
   * mends, and nothing a page can do — enrolling happens at a terminal with the
   * new Recovery Code — so the sentence is all a screen has to show: a retry
   * offered beside it can only meet it again.
   */
  | 'epoch'
  /**
   * Another device changed the Library while this one was writing to it, so
   * nothing was committed: a deletion, a freeze or a sync planned over a
   * Library that has since moved. Unlike `server` it is something a person can
   * act on — running the same thing again catches up first and plans from the
   * Library as it now stands — and the message says so.
   */
  | 'conflict'
  /**
   * The server is locked, so nothing that needs the Master Key can be done: the
   * Passphrase is required, and the message says how to give it.
   *
   * The opposite verdict to `unauthorized`, and about the opposite person. That
   * one is said to somebody who is not the owner of this Library and tells them
   * nothing; this is said to the owner about their own device and tells them
   * everything. Nothing on a page can undo it by itself — the Passphrase is
   * never typed into a page — so what a screen does with it is show the
   * sentence, which says where the Passphrase is entered for the process at
   * hand: in the desktop app's own window, which the page can ask the server to
   * bring forward (`POST /api/unlock`), or, for a server started from the
   * command line, by starting it again.
   *
   * Not to be confused with the `key_lost` in {@link PlacementReason}, which is
   * one Entry whose Container the Library records no key for and which no
   * Passphrase remedies. The two never arrive together: a locked server
   * declines nothing, because it fetches nothing.
   */
  | 'locked'
  | 'storage'
  | 'unverified'
  | 'server'
  /**
   * The request got no answer here: nothing is listening, the network went, or
   * the transfer broke while the body was still going up — which is
   * not proof the server never answered. Every `fetch` that rejects becomes
   * this, whatever it rejected for, except one the caller aborted: that is not
   * a refusal at all and passes through as itself. An answer whose status
   * arrived and whose body broke off before it could be read becomes this too.
   */
  | 'unreachable'
  /** Something answered, and it was not one of the shapes above. */
  | 'unrecognized';

/**
 * Which way a placement was declined or refused, where one was — and, under
 * `storage`, the one reason that is about Storage rather than a placement.
 *
 * One vocabulary across the kinds that carry it. A fetch's `declined`
 * carries `unmapped`, `unmaterializable`, `reserved`, `surfaced` and `key_lost`,
 * and a drop meets `unmaterializable` and `reserved` the same way. A
 * `refused_placement` carries `refused_root`, `unmapped` for a drop or a
 * freeze under a folder this device has no folder for, and `pack_resident`,
 * which is a drop's alone: the Library holds an Entry at that path inside a
 * Pack, and coffret cannot replace one of those yet — so the file is refused
 * rather than written where no sync could carry it in.
 */
export type PlacementReason =
  | 'unmapped'
  | 'unmaterializable'
  /**
   * The path carries a name coffret keeps for itself inside a mapped folder:
   * the management area its own bookkeeping lives in, or the scratch a
   * half-written file is called by. A scan passes both over, so a file placed
   * under either would sit in the folder and never reach the Library.
   *
   * The management area's name is matched with ASCII case folded, and a
   * spelling that only folds to it is refused rather than passed over: a drop,
   * a read of one file, and a listing of a folder holding one are the three
   * declined this way. A sync or a freeze that stops at such a name is not one
   * of them — it reaches a page as a run that stopped, with no name in it. So
   * the name may be `.COFFRET` rather than `.coffret`, and it may stand in the
   * folder that was asked for rather than in the path itself.
   */
  | 'reserved'
  /**
   * A folder this device maps is not the folder its mapping was recorded
   * against — a copied disk, a mount that came back different — so nothing was
   * placed into it. Nothing on a page remedies it: the message names the
   * gesture, and it is one at a terminal.
   */
  | 'refused_root'
  | 'surfaced'
  | 'key_lost'
  | 'pack_resident'
  /**
   * Not a placement's reason at all, and the one reason a `storage` refusal
   * carries: Storage no longer takes this device's grant — on Google Drive, a
   * permission that ran out (every seven days, for a consent screen in testing)
   * or was revoked. Pressing the same control again will never clear it; a
   * reconnect does (`POST /api/reconnect`), so this is what a screen offers one
   * from. Every other `storage` refusal carries no reason.
   */
  | 'unauthenticated';

/**
 * The finding about one Entry, by the name the device layer gives it: what a
 * declined fetch reported, and what a run's finding reports in its field of the
 * same name.
 *
 * The last two only a sync finds, so they arrive on a finding and never on a
 * refusal: nothing a fetch does is declined over a file that changed inside a
 * Pack or one this device no longer has.
 */
export type SurfacedFinding =
  | 'ForeignFile'
  | 'LocallyChanged'
  | 'WitnessedDeletion'
  /**
   * A folder on the way to where the file belongs is not a folder of the mapped
   * folder — a symbolic link, or an ordinary file standing where a folder must
   * be. The rest of a run is unaffected: this is the shape of one folder.
   */
  | 'UnreachablePlace'
  | 'KeyLost'
  /**
   * The Entry's path carries `.coffret`, or a name differing from it only in
   * ASCII case, which is the name reserved for the device's own folder inside a
   * mapped folder at any depth. A placement refuses the two alike, and is the
   * one place they are one finding: a scan steps over the exact name and stops
   * at a fold of it. Nothing is placed there: a file
   * under it is one no later scan looks at, and one at the marker inside it
   * would take the mapped folder's identity away. No scan of this device makes
   * such a path, so it came from whichever device committed it.
   */
  | 'ReservedComponent'
  /** The file changed, and the Entry it changed from is inside a Pack. */
  | 'ChangedInPack'
  /** This device had the file and it is gone; the Library still holds it. */
  | 'DeletedLocally';

/**
 * What a placement under a folder this device has no folder for is told as
 * (spec: EP-9): the sentence the server declines a fetch and refuses a drop
 * with there.
 *
 * Here as well as in the server's answers because one gesture meets it without
 * asking: clicking a row of a folder no mapping reaches makes no request, so the
 * page says it for the server, in the server's words. The contract case holds
 * this to the refusals the server sends. It is a clause rather than a sentence —
 * lower-case, and unpunctuated at the end — so a screen sets it inside one of
 * its own.
 */
export const NO_FOLDER_HERE = 'no folder on this device holds this part of the Library';

/**
 * One refusal, in the shape every refusal this client hands on takes.
 *
 * The same four fields whether the refusal came back instead of an answer —
 * thrown as a {@link Refusal} — or inside one: a fill's declined Entry, what
 * stopped a run, a part a drop refused. The server sends all of them in one
 * shape, and this is that shape read: `error` becomes `kind`, and a reason or a
 * finding name this client has not heard of becomes `null`, exactly as on the
 * request path, so a screen compares one with the other without a second
 * vocabulary between them.
 */
export interface Refused {
  /** Which kind of refusal this is, for the caller to branch on. */
  readonly kind: RefusalKind;
  /** The server's own sentence, written to be read by a person. */
  readonly message: string;
  /**
   * Present wherever the kind is `declined` or `refused_placement`, and on a
   * `storage` refusal only where it is `unauthenticated`.
   */
  readonly reason: PlacementReason | null;
  /** Present where the reason is `surfaced` or `key_lost`. */
  readonly surfaced: SurfacedFinding | null;
}

/**
 * Everything that can come back instead of an answer, in one shape.
 *
 * An `Error` so that it travels the way a failed request already does — thrown
 * out of the call, caught where the screen decides what to show — and a typed
 * one so that the screen can branch. `message` is the server's own sentence and
 * is written to be read by a person, which is why it is the one thing a caller
 * may display verbatim.
 */
export class Refusal extends Error implements Refused {
  readonly kind: RefusalKind;
  /**
   * The HTTP status, and `0` where no answer arrived at all. An `unreachable`
   * whose answer broke off after its status arrived keeps that status.
   */
  readonly status: number;
  readonly reason: PlacementReason | null;
  readonly surfaced: SurfacedFinding | null;
  /**
   * The Entry Paths a drop had already written when it was stopped, and `null`
   * on every refusal that is not a drop stopped part way.
   *
   * A drop refused as a whole — a budget passed, a device out of room — leaves
   * what landed before it in the folder, whole and with nothing armed to carry
   * it in. The sentence says why the drop stopped; this says what of it is
   * there, which is what a screen reloads the folder to show.
   */
  readonly written: readonly string[] | null;

  constructor(
    kind: RefusalKind,
    status: number,
    message: string,
    reason: PlacementReason | null = null,
    surfaced: SurfacedFinding | null = null,
    written: readonly string[] | null = null,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = 'Refusal';
    this.kind = kind;
    this.status = status;
    this.reason = reason;
    this.surfaced = surfaced;
    this.written = written;
  }
}

/** Whether something thrown out of this client is one of its refusals. */
export function isRefusal(thrown: unknown): thrown is Refusal {
  return thrown instanceof Refusal;
}

/**
 * The refusal one non-2xx answer stands for.
 *
 * It never throws, whatever came back. A body that is not the server's JSON is
 * an ordinary thing to receive — a proxy's own error page stands where the
 * server would have been — and a parser that threw there would replace a
 * refusal a caller can show with one it cannot.
 */
export async function refusalOf(response: Response): Promise<Refusal> {
  // Read whole before it is parsed, because the two ways of not getting a
  // refusal out of it are different things to say. A body that stopped
  // arriving is this server's answer, broken off — the status is its own, and
  // most often it is a drop refused while the browser was still sending it. A
  // body that arrived and is not the server's shape is somebody else replying.
  // Parsing straight off the stream would say the second about the first.
  let text: string;
  try {
    text = await response.text();
  } catch (cause) {
    return new Refusal(
      'unreachable',
      response.status,
      `the coffret server answered ${response.status}, and the answer broke off before it could be read`,
      null,
      null,
      null,
      { cause },
    );
  }
  const body = parsed(text);
  if (body === null) {
    // Said the way the person meets it: they asked the coffret server and no
    // coffret answer came back — whoever wrote this page, a proxy most of the
    // time, is not who they were asking. The status stays in the sentence
    // because it is the one clue for whoever then goes looking.
    return new Refusal(
      'unrecognized',
      response.status,
      `the coffret server did not answer — something else replied ${response.status} in its place`,
    );
  }
  const { kind, message, reason, surfaced } = body.refused;
  return new Refusal(kind, response.status, message, reason, surfaced, body.written);
}

/** A refusal body, read: the four fields every refusal has, and `written`. */
interface RefusalBody {
  refused: Refused;
  written: readonly string[] | null;
}

/** The body as the server's refusal shape, or `null` where it is not one. */
function parsed(text: string): RefusalBody | null {
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    return null;
  }
  const refused = refusedIn(body);
  if (refused === null) {
    return null;
  }
  return { refused, written: writtenOf((body as Record<string, unknown>).written) };
}

/**
 * The refusal one answer carries inside it — what stopped a run, an Entry a
 * fill declined, a part a drop refused — read as the request path reads one.
 *
 * The same narrowing a refusal thrown instead of an answer goes through, so a
 * kind this client has not heard of is `unrecognized` and a reason or a finding
 * name it has not heard of is `null` wherever the refusal arrived. Without it the
 * answer's refusals would reach a screen as whatever strings the server sent,
 * merely claiming the unions they are typed as — and a comparison against one of
 * those unions would be answered by a value outside it.
 *
 * It never throws, for the reason {@link refusalOf} does not: something that is
 * not a refusal at all is `unrecognized`, with a sentence saying so.
 */
export function refusedOf(value: unknown): Refused {
  return (
    refusedIn(value) ?? {
      kind: 'unrecognized',
      message: 'the coffret server answered with a refusal this page cannot read',
      reason: null,
      surfaced: null,
    }
  );
}

/** `value` read as the four fields of a refusal, or `null` where it is not one. */
function refusedIn(value: unknown): Refused | null {
  if (typeof value !== 'object' || value === null) {
    return null;
  }
  const fields = value as Record<string, unknown>;
  if (typeof fields.error !== 'string' || typeof fields.message !== 'string') {
    return null;
  }
  return {
    kind: kindOf(fields.error),
    message: fields.message,
    reason: reasonOf(fields.reason),
    surfaced: surfacedOf(fields.surfaced),
  };
}

/**
 * What a stopped drop says had landed, and `null` where the answer said nothing
 * about it — or said it in a shape that is not a list of paths, which a screen
 * is better off treating as silence than trusting.
 */
function writtenOf(field: unknown): readonly string[] | null {
  if (!Array.isArray(field) || !field.every((path) => typeof path === 'string')) {
    return null;
  }
  return field as string[];
}

const KINDS: readonly string[] = [
  'bad_path',
  'bad_request',
  'unauthorized',
  'no_such_entry',
  'no_such_route',
  'declined',
  'refused_placement',
  'epoch',
  'conflict',
  'locked',
  'storage',
  'unverified',
  'server',
];

const REASONS: readonly string[] = [
  'unmapped',
  'unmaterializable',
  'reserved',
  'refused_root',
  'surfaced',
  'key_lost',
  'pack_resident',
  'unauthenticated',
];

/**
 * The finding names the server can send, read from the one file that holds
 * them.
 *
 * Not written out here, because a list written out here is a copy: the names
 * are the server's, spelled in its own `name_of` for a refusal and in
 * `finding.rs`'s `named` for a run's finding, and a variant renamed there
 * would leave this reading the new name as `null` and every screen showing the
 * generic sentence with nothing to say something had gone wrong. The file is
 * what the cases in `coffret-server` build from those `match`es and compare
 * against — a refusal's names at its head, a finding's the whole of it — so a
 * rename fails `cargo test` until the file is brought along, and this follows
 * the file with no second list to forget.
 *
 * {@link SurfacedFinding} stays written out, and is not a second copy of this:
 * it is the compile-time shape a caller branches on, checked where the branch
 * is written. What nothing checks is that union against the file — the file is
 * an array of strings, so a name it grew and the union did not is cast into the
 * union below and reaches a `switch` with no case for it. So a finding renamed
 * on the server is renamed in the file and in the union together, and each
 * case in `coffret-server` that compares its names with the file asks for both
 * where it fails.
 */
const FINDINGS: readonly string[] = surfacedFindings;

/**
 * The kind the body named, and `unrecognized` for one this client has not heard
 * of.
 *
 * A server that grew a kind is not a server this client can branch on, and
 * saying so is better than passing a string on as though it were one of the
 * thirteen: a caller matching on the union would then fall through every case.
 */
function kindOf(named: string): RefusalKind {
  return KINDS.includes(named) ? (named as RefusalKind) : 'unrecognized';
}

/** The reason the body named, and `null` for none or for one this client has not heard of. */
function reasonOf(named: unknown): PlacementReason | null {
  return typeof named === 'string' && REASONS.includes(named) ? (named as PlacementReason) : null;
}

/**
 * The finding name the body named, and `null` for none or for one this client
 * has not heard of.
 *
 * Exported for a run's findings, which name one by the same field and the same
 * names a declined fetch does: the one list of names reads both.
 */
export function surfacedOf(named: unknown): SurfacedFinding | null {
  return typeof named === 'string' && FINDINGS.includes(named) ? (named as SurfacedFinding) : null;
}
