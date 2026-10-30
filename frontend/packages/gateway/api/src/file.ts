import { apiUrl, asked } from './request';

/**
 * The URL one Entry's plaintext is served at — `GET /api/file?path=`.
 *
 * The one place this route's URL is spelled. The path travels as a query
 * parameter rather than a path segment because an Entry Path's only logical
 * separator is `/`, and a segment would have to escape every one of them.
 */
export function fileUrl(path: string): string {
  return apiUrl('file', { path });
}

/**
 * One Entry's plaintext, fetched into place first where this device does not
 * have it.
 *
 * Fetched rather than pointed at with an `<img src>`, for what a failure has to
 * say: the answer to a request that could not be served is a refusal with a
 * sentence in it, and an `<img>` that will not load has no sentence — only that
 * something went wrong. The caller gets the bytes and decides what to do with
 * them.
 */
export async function getFile(path: string, signal?: AbortSignal): Promise<Blob> {
  const response = await asked(fileUrl(path), signal);
  return response.blob();
}

/**
 * One file's bytes where they are on this device already, and a refusal where
 * they are not — `GET /api/file?path=&peek=true`.
 *
 * What a preview asks for: the server answers from the file it placed or the
 * one somebody added, and neither fetches from Storage nor sets the rest of the
 * folder coming, as opening a page does. Looking at a row is not opening it.
 */
export async function peekFile(path: string, signal?: AbortSignal): Promise<Blob> {
  const response = await asked(apiUrl('file', { path, peek: 'true' }), signal);
  return response.blob();
}

/**
 * One Entry's plaintext, fetched into place first where this device does not
 * have it, without setting the rest of its folder coming —
 * `GET /api/file?path=&fill=false`.
 *
 * What the reader reads ahead of itself with: the first pages of the folder
 * it is about to read on into, and a folder's cover. Reading those with
 * [`getFile`](#getFile) would bring the whole folder over, and a fill of the
 * next folder stops the fill of the one still being read; this brings the
 * one file and leaves that fill running. The fill of a folder read ahead of
 * starts once one of its pages is read with `getFile`.
 */
export async function readAhead(path: string, signal?: AbortSignal): Promise<Blob> {
  const response = await asked(apiUrl('file', { path, fill: 'false' }), signal);
  return response.blob();
}
