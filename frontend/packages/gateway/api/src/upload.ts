import type { Refused } from './activity';
import { apiUrl, askedForJson } from './request';

/** One file on its way into a folder, and where it goes inside it. */
export interface Added {
  /**
   * Its path relative to the folder it is being added to.
   *
   * `photo.jpg` for a file added on its own, `holiday/day1/photo.jpg` for one
   * inside a dropped folder — the folders being what the separators mean, and
   * what the server makes on the way.
   */
  path: string;
  file: File;
}

/**
 * One part the server did not write, and why.
 *
 * A `Refused` under the name it was sent by, as an Entry a fill declined is one
 * under its Entry Path: the server answers all three in the same four fields, so
 * a screen reads them with the branches it already has rather than with a second
 * vocabulary that could drift from the first.
 */
export interface RefusedPart extends Refused {
  /** The relative path it was sent under, which may not be a path at all. */
  name: string;
}

/**
 * What became of one drop — `POST /api/upload?path=`.
 *
 * Per part, because a drop is a handful of files and they are separate
 * questions: one name the Library holds inside a Pack does not stop the file
 * beside it landing.
 *
 * What is not a separate question is the folder they are going into, or the
 * folder on the device that stands for it. A drop onto a folder goes through one
 * of each, so what is refused about either is refused of the whole request:
 * there is no answer of this shape at all, and the refusal is thrown out of
 * {@link addFiles} instead.
 */
export interface Upload {
  /** The Entry Paths the files were written at, in the order they arrived. */
  written: string[];
  /** The parts nothing was written for. */
  refused: RefusedPart[];
}

/** How one drop is being made, beyond which folder it is onto. */
export interface Adding {
  /**
   * Whether this drop is a book being brought into a folder made for it.
   *
   * The server packs such a drop rather than syncing it: the pages go up once,
   * as Packs, instead of as one Container per page — which for a scanned book is
   * the difference between a handful of Storage objects and several hundred.
   *
   * It is stated rather than worked out, and not worked out here either: only
   * the screen knows that the folder being dropped onto is one the person made a
   * moment ago and has not filled yet. Left out, the drop is the ordinary one
   * and the server syncs it.
   */
  freeze?: boolean;
  signal?: AbortSignal;
}

/**
 * Adds files to one folder of the Library; the empty string is the Library root.
 *
 * One request for the whole drop, each file a part whose filename is its path
 * relative to the folder. That is what lets a folder drop and a plain file drop
 * be the same request: the separators in a part's name are the folders, and the
 * server makes them.
 *
 * The body is a `FormData`, so the browser streams the files rather than this
 * client reading them into memory — a drop of a hundred photographs is a
 * hundred file handles and not a hundred copies.
 *
 * A refusal thrown out of this is about the drop as a whole, and two of them are
 * about where it was going. `unmapped`: no mapping of this device reaches the
 * folder, so there is nowhere to put any of it. `refused_root`: a mapping does
 * reach it, and the folder on the device is not the one that mapping was
 * recorded against — so this device has somewhere to put it and will not write
 * there until the mapping is recorded again. Either way nothing of the drop is
 * written into that folder.
 *
 * The others are not about where it was going but about what it costs: a budget
 * of the server's that the drop passed — how much one request may carry, how
 * much one part of it may, how many parts there may be — or a device that has
 * not the room for what is still coming. Any of those may be met after part of
 * the drop has landed, and then those parts are in the folder with nothing
 * armed to carry them in.
 *
 * All of them are answered while the browser may still be sending the body, so
 * a transfer that fails before the answer is read is thrown out as
 * `unreachable` rather than as the refusal: `unreachable` out of this function
 * is not proof the server is gone.
 *
 * What was refused about one file is in the answer, beside what landed.
 */
export function addFiles(
  folder: string,
  files: Added[],
  adding: Adding = {},
): Promise<Upload> {
  const body = new FormData();
  for (const added of files) {
    // The name of the field is not read by anything: what the server takes is
    // the filename, which is where the file goes.
    body.append('file', added.file, added.path);
  }
  const params: Record<string, string> = {};
  if (folder !== '') {
    params.path = folder;
  }
  if (adding.freeze === true) {
    params.freeze = 'true';
  }
  return askedForJson<Upload>(
    apiUrl('upload', params),
    adding.signal,
    'POST',
    body,
  );
}
