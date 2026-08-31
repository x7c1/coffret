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
 */
export interface Upload {
  /** The Entry Paths the files were written at, in the order they arrived. */
  written: string[];
  /** The parts nothing was written for. */
  refused: RefusedPart[];
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
 * A refusal thrown out of this is about the drop as a whole: the folder is not
 * on this device, so there is nowhere to put any of it. What was refused about
 * one file is in the answer, beside what landed.
 */
export function addFiles(
  folder: string,
  files: Added[],
  signal?: AbortSignal,
): Promise<Upload> {
  const body = new FormData();
  for (const added of files) {
    // The name of the field is not read by anything: what the server takes is
    // the filename, which is where the file goes.
    body.append('file', added.file, added.path);
  }
  return askedForJson<Upload>(
    apiUrl('upload', folder === '' ? undefined : { path: folder }),
    signal,
    'POST',
    body,
  );
}
