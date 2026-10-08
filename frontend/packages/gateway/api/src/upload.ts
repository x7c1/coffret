import type { Refused } from './refusal';
import { Refusal, refusedOf } from './refusal';
import { apiUrl, sentForJson, type Progress } from './request';
import uploadBudget from './upload-budget.json';

/**
 * The three budgets one drop is taken within (spec: LA-9): how much the whole
 * request may carry, framing included; how much one part — one file — may; and
 * how many parts there may be.
 *
 * All three are the server's and none is written out here: the file is what the
 * cases in `coffret-server` hold to the budgets that server is mounted with, so
 * a change on that side fails `cargo test` until the file follows it, and this
 * follows the file.
 */
const REQUEST_BUDGET: number = uploadBudget.request_bytes;
const PART_BUDGET: number = uploadBudget.part_bytes;
const PARTS_BUDGET: number = uploadBudget.parts;

/**
 * Which of the server's budgets a drop is past, worked out from what its files
 * say they are before any of them is sent.
 *
 * - `part`: one file is larger than one part may be. `name` is the path it
 *   would have been sent under, and `size` what it says it is.
 * - `parts`: the drop holds more files than one request may carry parts for
 *   (one part per file).
 * - `request`: the files come to more than one request may carry.
 *
 * `limit` is the budget passed, in the unit the other number is in.
 */
export type Overdrawn =
  | { budget: 'part'; name: string; size: number; limit: number }
  | { budget: 'parts'; count: number; limit: number }
  | { budget: 'request'; carried: number; limit: number };

/**
 * A drop refused before any of it was sent, because the server was certain to
 * refuse it.
 *
 * A {@link Refusal} like every other, so it travels and is caught the way they
 * are, and `written` is empty, because nothing was. What it adds is
 * {@link Overdrawn}: which budget, and by how much — the facts a screen says it
 * with, in its own units and beside the Library's name, which this package has
 * neither of. The message is the reason alone, for a screen that has nothing
 * better to say.
 */
export class OverBudget extends Refusal {
  readonly overdrawn: Overdrawn;

  constructor(overdrawn: Overdrawn) {
    super('bad_request', 413, reasonOf(overdrawn), null, null, []);
    this.name = 'OverBudget';
    this.overdrawn = overdrawn;
  }
}

/** Whether something thrown out of {@link addFiles} is a drop refused before sending. */
export function isOverBudget(thrown: unknown): thrown is OverBudget {
  return thrown instanceof OverBudget;
}

/** The reason an overdrawn drop is refused for, in bytes and counts as they are. */
function reasonOf(overdrawn: Overdrawn): string {
  switch (overdrawn.budget) {
    case 'part':
      return `${overdrawn.name} is ${overdrawn.size} bytes, more than the ${overdrawn.limit} one file can be when dropped`;
    case 'parts':
      return `this drop holds ${overdrawn.count} files, more than the ${overdrawn.limit} one drop can carry`;
    case 'request':
      return `this drop comes to ${overdrawn.carried} bytes, more than the ${overdrawn.limit} one drop can carry`;
  }
}

/**
 * The budget a drop of `files` is certain to be refused by, or `null` where the
 * server is the one to weigh it.
 *
 * One file past the part budget is said before a count past the parts budget,
 * and both before the request budget: the first names the one file to take out,
 * and halving a drop that holds it would be halving one that is refused again.
 *
 * Each is a boundary. A file of exactly the part budget is the server's to take,
 * and so is a drop of exactly as many files as there may be parts — the
 * explorer sends one part per file and nothing else. Files that come to exactly
 * the request budget are the server's to weigh too, because the framing on top
 * is what decides, and only the server sees the body the browser makes.
 */
function overdrawnBy(files: readonly Added[]): Overdrawn | null {
  const large = files.find((added) => added.file.size > PART_BUDGET);
  if (large !== undefined) {
    return { budget: 'part', name: large.path, size: large.file.size, limit: PART_BUDGET };
  }
  if (files.length > PARTS_BUDGET) {
    return { budget: 'parts', count: files.length, limit: PARTS_BUDGET };
  }
  const carried = files.reduce((sum, added) => sum + added.file.size, 0);
  if (carried > REQUEST_BUDGET) {
    return { budget: 'request', carried, limit: REQUEST_BUDGET };
  }
  return null;
}

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
 * under its Entry Path: the server answers all three in the same four fields,
 * and they are read by the same narrowing, so a screen reads them with the
 * branches it already has rather than with a second vocabulary that could drift
 * from the first.
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
  /**
   * Told how much of the request has gone as the browser sends it: `sent` of
   * `total` bytes of the body, framing included.
   *
   * For the wait a drop is before the server has anything to say: a book of
   * several hundred pages is tens of megabytes, and until the last of it is
   * sent there is no sync or freeze yet to report its own phases.
   */
  onProgress?: Progress;
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
 * And it says how much of that body has gone, through `onProgress`, which is
 * why this one request is made with an `XMLHttpRequest` rather than `fetch`:
 * `fetch` reports nothing about a request while it is being sent. It keeps the
 * contract every other request in this package keeps; [`sentForJson`](./request)
 * says how.
 *
 * The bytes are the first half of the wait and not the whole of it: once the
 * body is sent the server arms the sync or the freeze that carries the files
 * into the Library, and that reports its own phases through the work answer.
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
 * not the room for what is still coming. Any of those may be met after some of
 * the drop has landed, and then those parts are in the folder with nothing
 * armed to carry them in.
 *
 * All of them are answered while the browser may still be sending the body, so
 * a transfer that fails before the answer is read is thrown out as
 * `unreachable` rather than as the refusal: `unreachable` out of this function
 * is not proof the server is gone.
 *
 * The budgets are not left to that. Where one file handed in is larger than
 * one part may be, where there are more files than one request may carry parts
 * for, or where the files already come to more than one request may carry, the
 * server's refusal is certain — and it would come while the browser was still
 * sending, after some of the drop may have landed, which is when a browser is
 * likeliest to report it as a transfer that broke. So it is refused here,
 * before anything is sent, as an {@link OverBudget} saying which budget it
 * passed ({@link overdrawnBy}); `written` is empty, because nothing was.
 *
 * What was refused about one file is in the answer, beside what landed. And a
 * refusal of the whole drop read off an answer that did arrive carries
 * `written`: what had landed before it stopped.
 */
export async function addFiles(
  folder: string,
  files: Added[],
  adding: Adding = {},
): Promise<Upload> {
  const overdrawn = overdrawnBy(files);
  if (overdrawn !== null) {
    throw new OverBudget(overdrawn);
  }
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
  return uploadOf(
    await sentForJson<unknown>(apiUrl('upload', params), body, adding.signal, adding.onProgress),
  );
}

/**
 * One upload answer, read: each refused part's refusal through the narrowing a
 * refused request goes through, so a kind, a reason or a finding name this
 * client has not heard of lands where it lands there.
 */
export function uploadOf(sent: unknown): Upload {
  const upload = sent as { written: string[]; refused: unknown[] };
  return {
    written: upload.written,
    refused: upload.refused.map((part) => ({
      name: (part as { name: string }).name,
      ...refusedOf(part),
    })),
  };
}
