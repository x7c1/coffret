import { apiUrl, askedForJson } from './request';

/**
 * What became of the folder's identity: `written` where it carried none,
 * `adopted` where it already carried one, which was kept. `reset` is the
 * command line's alone and never comes back from here.
 */
export type MappedMarker = 'written' | 'adopted' | 'reset';

/** What recording a mapping did — what `coffret map` says, as fields. */
export interface Mapped {
  /** The top-level folder of the Library mapped, and `null` for its root. */
  prefix: string | null;
  /** The folder on this device it now points at, as recorded. */
  local_root: string;
  /**
   * Where the same prefix pointed before, where it was already mapped:
   * everything under that folder has just left the Library's reach on this
   * device, which is worth saying.
   */
  replaced: string | null;
  /** What became of the folder's identity. */
  marker: MappedMarker;
  /** The server's own sentence, written to be read by a person. */
  message: string;
}

/**
 * Records that a folder on this device holds part of the Library —
 * `POST /api/map`.
 *
 * `prefix` is one top-level folder of the Library, or `null` for the Library
 * root. A folder that is not there or not a folder is a `bad_request` refusal
 * naming it, a prefix of more than one level is `bad_path`, and a folder whose
 * own management area stops the mapping is `bad_request` at `409`.
 *
 * The body is JSON, and is sent as a `Blob` that says so: the server reads only
 * a body declaring itself JSON, and a `Blob`'s type is what a browser sends as
 * the `Content-Type` of a body handed to it as one.
 */
export function mapFolder(
  localRoot: string,
  prefix: string | null,
  signal?: AbortSignal,
): Promise<Mapped> {
  const body = new Blob([JSON.stringify({ local_root: localRoot, prefix })], {
    type: 'application/json',
  });
  return askedForJson<Mapped>(apiUrl('map'), signal, 'POST', body);
}
