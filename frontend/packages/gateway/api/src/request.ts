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
export function apiUrl(route: string, params?: Record<string, string>): string {
  const query = new URLSearchParams(params).toString();
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
 * The method is here because one route is not a `GET`: asking the server to
 * take a folder up again is asking it to go and do something rather than to say
 * what it knows. There is no body on either — everything these routes take,
 * they take as `?path=`.
 */
export async function asked(
  url: string,
  signal?: AbortSignal,
  method: 'GET' | 'POST' = 'GET',
): Promise<Response> {
  let response: Response;
  try {
    response = await fetch(url, { method, signal });
  } catch (cause) {
    if (signal?.aborted === true) {
      throw cause;
    }
    throw new Refusal('unreachable', 0, 'the coffret server did not answer', null, null, {
      cause,
    });
  }
  if (!response.ok) {
    throw await refusalOf(response);
  }
  return response;
}

/**
 * The JSON one route answered with.
 *
 * The type is this package's word for the server's serialization and is not
 * checked at runtime: what would be checked is a contract both halves of this
 * repository are built from, and a validator here would be a second statement
 * of it to keep in step. A body that is not JSON at all is another matter — that
 * is something other than the server answering — and becomes a refusal.
 */
export async function askedForJson<T>(
  url: string,
  signal?: AbortSignal,
  method: 'GET' | 'POST' = 'GET',
): Promise<T> {
  const response = await asked(url, signal, method);
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
      { cause },
    );
  }
}
