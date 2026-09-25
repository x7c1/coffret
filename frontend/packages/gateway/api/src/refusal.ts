import surfacedFindings from './surfaced-findings.json';

/**
 * Which kind of refusal an answer is.
 *
 * The first eleven are the server's own, and the whole set is named here for the
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
  | 'declined'
  /**
   * This device has to be enrolled in the Library again: a Master Key epoch was
   * activated, and the device holds only the key it replaced. Nothing a retry
   * mends, and nothing a page can do — enrolling happens at a terminal with the
   * new Recovery Code — so the sentence is all a screen has to show: a retry
   * offered beside it can only meet it again.
   */
  | 'epoch'
  /**
   * The server is locked, so nothing that needs the Master Key can be done: the
   * Passphrase is required, and the message says how to give it.
   *
   * The opposite verdict to `unauthorized`, and about the opposite person. That
   * one is said to somebody who is not the owner of this Library and tells them
   * nothing; this is said to the owner about their own device and tells them
   * everything. Nothing on a page can undo it — the Passphrase is typed at a
   * terminal — so what a screen does with it is show the sentence.
   *
   * Not to be read as the `locked` in {@link DeclinedReason}, which is one
   * Entry whose Container the Library records no key for and which no
   * Passphrase resolves. The two never arrive together: a locked server
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
   * a refusal at all and passes through as itself.
   */
  | 'unreachable'
  /** Something answered, and it was not one of the shapes above. */
  | 'unrecognized';

/**
 * Which way something was declined, where it was.
 *
 * The first six are a fetch's, and a drop meets `unmapped`,
 * `unmaterializable`, `reserved` and `refused_root` as well. The last is a
 * drop's alone: the Library holds an Entry at that path inside a Pack, and
 * coffret cannot replace one of those yet — so the file is refused rather than
 * written where no sync could carry it in.
 */
export type DeclinedReason =
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
   * placed into it. Nothing on a page settles it: the message names the gesture,
   * and it is one at a terminal.
   */
  | 'refused_root'
  | 'surfaced'
  | 'locked'
  | 'pack_resident';

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
 * Everything that can come back instead of an answer, in one shape.
 *
 * An `Error` so that it travels the way a failed request already does — thrown
 * out of the call, caught where the screen decides what to show — and a typed
 * one so that the screen can branch. `message` is the server's own sentence and
 * is written to be read by a person, which is why it is the one thing a caller
 * may display verbatim.
 */
export class Refusal extends Error {
  /** Which kind of refusal this is, for the caller to branch on. */
  readonly kind: RefusalKind;
  /** The HTTP status, and `0` where no answer arrived at all. */
  readonly status: number;
  /** Present exactly where the kind is `declined`. */
  readonly reason: DeclinedReason | null;
  /** Present where the reason is `surfaced` or `locked`. */
  readonly surfaced: SurfacedFinding | null;

  constructor(
    kind: RefusalKind,
    status: number,
    message: string,
    reason: DeclinedReason | null = null,
    surfaced: SurfacedFinding | null = null,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = 'Refusal';
    this.kind = kind;
    this.status = status;
    this.reason = reason;
    this.surfaced = surfaced;
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
  const body = await parsed(response);
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
  return new Refusal(
    kindOf(body.error),
    response.status,
    body.message,
    reasonOf(body.reason),
    surfacedOf(body.surfaced),
  );
}

/** What a refusal looks like on the wire. */
interface RefusalBody {
  error: string;
  message: string;
  reason?: string;
  surfaced?: string;
}

/** The body as the server's refusal shape, or `null` where it is not one. */
async function parsed(response: Response): Promise<RefusalBody | null> {
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    return null;
  }
  if (typeof body !== 'object' || body === null) {
    return null;
  }
  const fields = body as Record<string, unknown>;
  if (typeof fields.error !== 'string' || typeof fields.message !== 'string') {
    return null;
  }
  return {
    error: fields.error,
    message: fields.message,
    reason: typeof fields.reason === 'string' ? fields.reason : undefined,
    surfaced: typeof fields.surfaced === 'string' ? fields.surfaced : undefined,
  };
}

const KINDS: readonly string[] = [
  'bad_path',
  'bad_request',
  'unauthorized',
  'no_such_entry',
  'no_such_route',
  'declined',
  'epoch',
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
  'locked',
  'pack_resident',
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
 * eleven: a caller matching on the union would then fall through every case.
 */
function kindOf(named: string): RefusalKind {
  return KINDS.includes(named) ? (named as RefusalKind) : 'unrecognized';
}

function reasonOf(named: string | undefined): DeclinedReason | null {
  return named !== undefined && REASONS.includes(named) ? (named as DeclinedReason) : null;
}

function surfacedOf(named: string | undefined): SurfacedFinding | null {
  return named !== undefined && FINDINGS.includes(named) ? (named as SurfacedFinding) : null;
}
