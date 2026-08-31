/**
 * Which kind of refusal an answer is.
 *
 * The first six are the server's own, and the whole set is named here for the
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
  | 'no_such_entry'
  | 'declined'
  | 'storage'
  | 'unverified'
  | 'server'
  /** The request never got an answer: nothing is listening, or the network went. */
  | 'unreachable'
  /** Something answered, and it was not one of the shapes above. */
  | 'unrecognized';

/** Which way a fetch was declined, where it was. */
export type DeclinedReason = 'unmapped' | 'unmaterializable' | 'surfaced' | 'locked';

/** The finding a declined fetch reported, by the name the device layer gives it. */
export type SurfacedFinding =
  | 'ForeignFile'
  | 'LocallyChanged'
  | 'WitnessedDeletion'
  | 'KeyLost';

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
    return new Refusal(
      'unrecognized',
      response.status,
      `the server answered ${response.status} in a shape this client does not know`,
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
  'no_such_entry',
  'declined',
  'storage',
  'unverified',
  'server',
];

const REASONS: readonly string[] = ['unmapped', 'unmaterializable', 'surfaced', 'locked'];

const FINDINGS: readonly string[] = [
  'ForeignFile',
  'LocallyChanged',
  'WitnessedDeletion',
  'KeyLost',
];

/**
 * The kind the body named, and `unrecognized` for one this client has not heard
 * of.
 *
 * A server that grew a kind is not a server this client can branch on, and
 * saying so is better than passing a string on as though it were one of the six:
 * a caller matching on the union would then fall through every case.
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
