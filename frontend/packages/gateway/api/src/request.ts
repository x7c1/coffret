import { Refusal, refusalOf } from './refusal';

/**
 * Where the routes are.
 *
 * Relative, and deliberately: the page and the server are one origin — the dev
 * server proxies `/api` to it, and a built bundle is served beside it — so
 * there is no host for this client to be configured with and no request of it
 * that could be aimed anywhere else.
 */
const BASE = '/api';

/** The URL of one route, with the query it is asked with. */
export function apiUrl(
  route: string,
  params?: Record<string, string> | readonly (readonly [string, string])[],
): string {
  const query = new URLSearchParams(params as Record<string, string> | string[][]).toString();
  return query === '' ? `${BASE}/${route}` : `${BASE}/${route}?${query}`;
}

/**
 * One answer, or a [`Refusal`] thrown.
 *
 * Every non-2xx becomes the typed refusal its body describes, and so does a
 * request that never got an answer at all: a caller has one thing to catch and
 * one sentence to show, whether the server refused or the server is not there.
 *
 * An abort is neither, and passes through as itself. A caller that cancelled its
 * own request has nothing to be told about it, and a screen that rendered
 * "aborted" as a refusal would be reporting its own tidying up as a failure.
 *
 * The method is here because not every route is a `GET`: asking the server to
 * take a folder up again, or to carry what was dropped into the Library, is
 * asking it to go and do something rather than to say what it knows.
 *
 * The body is here for the routes that have one. Everything else these routes
 * take, they take as `?path=` — a file being added cannot be said in a URL, and
 * neither, without escaping every separator of it, can the folder on this
 * device a mapping is recorded for. The files of a drop go up through
 * {@link sentForJson} instead, which keeps this contract and says how much of
 * the body has gone.
 */
export async function asked(
  url: string,
  signal?: AbortSignal,
  method: 'GET' | 'POST' = 'GET',
  body?: BodyInit,
): Promise<Response> {
  let response: Response;
  try {
    // No `Content-Type` of this client's own. A `FormData` body has a boundary
    // the browser mints as it serializes, and a header written here would name a
    // boundary that is not in the body — which the server then cannot find a
    // single part inside.
    response = await fetch(url, { method, signal, body });
  } catch (cause) {
    if (signal?.aborted === true) {
      throw cause;
    }
    throw new Refusal('unreachable', 0, noAnswer(body), null, null, null, { cause });
  }
  if (!response.ok) {
    const refusal = await refusalOf(response);
    // A body the caller stopped reading by aborting is the abort, not an
    // answer that broke off.
    if (refusal.kind === 'unreachable' && signal?.aborted === true) {
      throw refusal.cause;
    }
    throw refusal;
  }
  return response;
}

/**
 * What a request that got no answer says — a `fetch` that rejected, or an
 * `XMLHttpRequest` that errored — which depends on whether it was sending.
 *
 * A request with no body that got no answer really did get none: nothing was
 * listening, or the network went. One that was sending a body is another
 * matter. The server answers a drop it will not take while the browser is still
 * sending it (spec: LA-10), and a browser commonly reports that as a transfer
 * that failed rather than as the answer it was given — so "did not answer" is
 * the one thing it cannot honestly say. What it can say is that the transfer
 * broke, and that whatever came back could not be read.
 */
function noAnswer(body: BodyInit | XMLHttpRequestBodyInit | undefined): string {
  return body === undefined
    ? 'the coffret server did not answer'
    : 'the request broke off while it was being sent, so whatever the coffret server ' +
        'answered could not be read';
}

/**
 * The JSON one route answered with.
 *
 * The type is this package's word for the server's serialization and is not
 * checked at runtime: what would be checked is a contract both halves of this
 * repository are built from, and a validator here would be a second statement
 * of it to keep in step. A body that is not JSON at all is another matter — that
 * is something other than the server answering — and becomes a refusal.
 *
 * The one thing that is read rather than taken is the vocabulary of refusal an
 * answer carries inside it. The work answer and the upload's answer name kinds,
 * reasons and findings a screen branches on, so they are asked for as `unknown`
 * and read by the narrowing a refused request goes through (`workOf`,
 * `uploadOf`) rather than cast.
 */
export async function askedForJson<T>(
  url: string,
  signal?: AbortSignal,
  method: 'GET' | 'POST' = 'GET',
  body?: BodyInit,
): Promise<T> {
  const response = await asked(url, signal, method, body);
  try {
    return (await response.json()) as T;
  } catch (cause) {
    if (signal?.aborted === true) {
      throw cause;
    }
    throw new Refusal(
      'unrecognized',
      response.status,
      'the coffret server answered with something that is not JSON',
      null,
      null,
      null,
      { cause },
    );
  }
}

/**
 * How much of a request's body has gone: `sent` of `total` bytes.
 *
 * `total` is the length of the whole body the browser is sending, framing
 * included, and not what the files in it come to.
 */
export type Progress = (sent: number, total: number) => void;

/**
 * {@link askedForJson} for a `POST` whose body is worth watching go up.
 *
 * `fetch` reports nothing about a request while it is being sent, so this one
 * is made with an `XMLHttpRequest` and its `upload` events. Everything else
 * about it is the contract every other request in this package keeps, and is
 * kept by handing the answer to the same readers rather than by restating them:
 *
 * - the same URL and the same headers — none of this client's own, for the
 *   reason {@link asked} gives about a `FormData` boundary; the server's key is
 *   added by whatever serves the page, never by the page;
 * - a non-2xx answer becomes the refusal its body describes, read by
 *   {@link refusalOf} off a `Response` built from what arrived;
 * - a request that got no answer is `unreachable`, worded by what it was doing;
 * - an abort through `signal` passes through as the abort, and is not a
 *   refusal;
 * - an answer that is not JSON is `unrecognized`.
 *
 * `onProgress` is told every time the browser says more of the body has gone,
 * and only where the browser knows the whole length; pacing it for a screen is
 * the caller's business.
 */
export function sentForJson<T>(
  url: string,
  body: XMLHttpRequestBodyInit,
  signal?: AbortSignal,
  onProgress?: Progress,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    if (signal?.aborted === true) {
      reject(abortOf(signal));
      return;
    }
    const request = new XMLHttpRequest();
    const abort = () => request.abort();
    signal?.addEventListener('abort', abort, { once: true });
    const settle = (end: () => void) => {
      signal?.removeEventListener('abort', abort);
      end();
    };
    request.open('POST', url);
    request.responseType = 'text';
    if (onProgress !== undefined) {
      request.upload.addEventListener('progress', (event) => {
        if (event.lengthComputable) {
          onProgress(event.loaded, event.total);
        }
      });
    }
    request.addEventListener('abort', () => settle(() => reject(abortOf(signal))));
    // No answer at all: what `fetch` rejects with, said the way `asked` says it.
    const unanswered = (cause: unknown) =>
      settle(() =>
        reject(new Refusal('unreachable', 0, noAnswer(body), null, null, null, { cause })),
      );
    request.addEventListener('error', unanswered);
    request.addEventListener('load', () => {
      settle(() => {
        void answered<T>(request.status, request.responseText).then(resolve, reject);
      });
    });
    request.send(body);
  });
}

/**
 * What an aborted request rejects with: the signal's own reason, which is what
 * `fetch` rejects with, so a caller tells its own cancelling apart the same way
 * whichever transport carried the request.
 */
function abortOf(signal: AbortSignal | undefined): unknown {
  return signal?.reason ?? new DOMException('The operation was aborted.', 'AbortError');
}

/** One answer that arrived whole, read as {@link askedForJson} reads one. */
async function answered<T>(status: number, text: string): Promise<T> {
  if (status < 200 || status > 299) {
    let response: Response;
    try {
      response = new Response(text, { status });
    } catch {
      // A status no `Response` can be built with — outside `200`–`599`, or one
      // that may carry no body — is no answer this server gives to a `POST`,
      // and is said the way anything else replying in its place is.
      throw new Refusal(
        'unrecognized',
        status,
        `the coffret server did not answer — something else replied ${status} in its place`,
      );
    }
    throw await refusalOf(response);
  }
  try {
    return JSON.parse(text) as T;
  } catch (cause) {
    throw new Refusal(
      'unrecognized',
      status,
      'the coffret server answered with something that is not JSON',
      null,
      null,
      null,
      { cause },
    );
  }
}
