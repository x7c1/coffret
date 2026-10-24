// What a drop onto a folder carries, kept free of DOM so it is unit testable.
//
// A browser hands a drop over as items rather than as files: an item may be one
// file, or it may be a whole folder that has to be walked to find the files in
// it. Both end up as the same thing here — a file, and the path it takes
// relative to the folder it was dropped on — which is exactly what the upload
// route takes, so a folder drop and a plain file drop are one request.
//
// The walk is over an interface rather than over `FileSystemEntry`, and that is
// what makes it testable at all: the real entries satisfy it, and a case can
// hand it a tree of its own without a browser.
//
// In the desktop app none of that walk happens here. The explorer runs in the
// app's own window, whose preload offers `window.coffretDesktop`: the dropped
// files are handed to it, the app walks them on the device by their paths, and
// what comes back is a one-time drop handle and each file's path relative to
// where it was dropped from — never its bytes, and never a path on the device.
// The files become the same records a browser's walk makes, so the confirmation
// and the Pack choice are the same; only where their bytes come from differs.
//
// Either walk leaves out what an operating system keeps beside the person's
// files — `.DS_Store`, `Thumbs.db`, `._` files, a `__MACOSX` folder — by the
// rule a scan of a mapped folder reads (`systemFiles.ts`, spec: EP-17), and
// says so apart from what was not taken: such files are in almost every drop.

import { addedFile, type Added } from '@coffret/api';

import { isSystemFile, type SystemFile } from './systemFiles';
import { said } from './useAsked';

/** One thing a drop carried, as much of it as the walk needs. */
export interface DroppedEntry {
  readonly isFile: boolean;
  readonly isDirectory: boolean;
  /** Its last path component, which is what it is called. */
  readonly name: string;
}

/** One file in a drop. */
export interface DroppedFile extends DroppedEntry {
  file(onFile: (file: File) => void, onRefused?: (cause: unknown) => void): void;
}

/** One folder in a drop. */
export interface DroppedFolder extends DroppedEntry {
  createReader(): DroppedReader;
}

/**
 * What reads one folder's children.
 *
 * It answers a batch at a time and says it is finished by answering with none,
 * which is not a convention this code invented and not one it may skip: browsers
 * return the children of a large folder a hundred at a time, so a walk that read
 * once would silently add the first hundred files of a folder and none of the
 * rest.
 */
export interface DroppedReader {
  readEntries(
    onEntries: (entries: DroppedEntry[]) => void,
    onRefused?: (cause: unknown) => void,
  ): void;
}

/** What a browser's walk of a drop found: the files, and what it left out. */
export interface Walked {
  files: Added[];
  /** What an operating system keeps, in the order the walk met it (spec: EP-17). */
  systemFiles: SystemFile[];
}

/**
 * Every file under what was dropped, each with its path relative to the folder
 * it was dropped on.
 *
 * A name an operating system keeps is left out wherever it stands, a dropped
 * item itself included, and a volume's own folders directly in a dropped
 * folder — where a dropped volume's top holds them — and nothing under a folder
 * of one is walked. They are handed back apart, for the confirmation to say
 * quietly (spec: EP-17).
 *
 * Depth first and in the order the browser answered in, so the files of one
 * dropped folder stay together. The server sorts nothing by this — where a row
 * lands in the listing is its Entry Path's business — so the order here is only
 * the order the parts go up in and the order a refusal is reported in.
 *
 * A folder contributes its own name to everything under it and nothing of its
 * own: an empty folder adds no file, and therefore adds nothing. That is not a
 * loss — a Library has no folders to add, only Entry Paths whose separators
 * imply them — so there is nothing an empty one could be carried in as.
 */
export async function filesUnder(entries: DroppedEntry[]): Promise<Walked> {
  const walked: Walked = { files: [], systemFiles: [] };
  for (const entry of entries) {
    await collect(entry, '', walked);
  }
  return walked;
}

/** One entry and everything under it, appended to `walked`. */
async function collect(entry: DroppedEntry, under: string, walked: Walked): Promise<void> {
  const path = under === '' ? entry.name : `${under}/${entry.name}`;
  // Directly in a dropped folder is a name whose folder is the dropped item
  // itself, whose path is one component.
  const atTop = under !== '' && !under.includes('/');
  if ((entry.isFile || entry.isDirectory) && isSystemFile(entry.name, atTop)) {
    walked.systemFiles.push({ path, folder: entry.isDirectory });
    return;
  }
  if (entry.isFile) {
    walked.files.push(addedFile(path, await fileOf(entry as DroppedFile)));
    return;
  }
  if (!entry.isDirectory) {
    // Neither a file nor a folder: a browser answers this for a drop that is not
    // a filesystem item at all — a selection of text, an image dragged out of
    // another page. There is nothing to add.
    return;
  }
  for (const child of await childrenOf(entry as DroppedFolder)) {
    await collect(child, path, walked);
  }
}

/** The file one entry stands for. */
function fileOf(entry: DroppedFile): Promise<File> {
  return new Promise((resolve, reject) => {
    entry.file(resolve, reject);
  });
}

/** Every child of one folder, however many batches it takes. */
async function childrenOf(entry: DroppedFolder): Promise<DroppedEntry[]> {
  const reader = entry.createReader();
  const children: DroppedEntry[] = [];
  for (;;) {
    const batch = await new Promise<DroppedEntry[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    if (batch.length === 0) {
      return children;
    }
    children.push(...batch);
  }
}

/**
 * What a drop carried, as the browser handed it over.
 *
 * The entries are asked for first, because they are the only way a dropped
 * folder can be walked at all: `dataTransfer.files` holds the folder as one
 * unreadable item. Where a browser offers no entry for an item — it is not a
 * filesystem item — the item is left out, and a drop of nothing but those is an
 * empty answer rather than a request.
 */
export async function droppedFiles(transfer: DataTransfer): Promise<Drop> {
  const bridge = desktopBridge();
  if (bridge !== null) {
    // Read before anything is awaited, as the entries below are: what the
    // event carries is gone once it has been handled.
    return heldBy(bridge, Array.from(transfer.files));
  }
  const entries: DroppedEntry[] = [];
  for (const item of Array.from(transfer.items)) {
    const entry = item.webkitGetAsEntry();
    if (entry !== null) {
      entries.push(entry);
    }
  }
  // A browser's walk leaves nothing out that it could say beside what the
  // system keeps: what it cannot read refuses the whole walk, and what is not a
  // filesystem item has no name.
  const walked = await filesUnder(entries);
  return { ...walked, notTaken: [] };
}

/**
 * What one drop carried: the files it is added as, what the desktop app met in
 * it and did not take — none in a browser, whose walk takes everything it can
 * read or refuses the drop — and, apart from both, what either walk left out
 * because an operating system keeps it.
 */
export interface Drop {
  files: Added[];
  /** What was not taken, in the order the walk met it; never an empty path. */
  notTaken: NotTaken[];
  /** What the system keeps, which is said quietly and never as not taken (spec: EP-17). */
  systemFiles: SystemFile[];
}

/** One file the desktop app took, as its answer names it. */
export interface HeldFile {
  /** Its path relative to the folder it was dropped from. */
  path: string;
  size: number;
  /** Its modification time, in milliseconds from the Unix epoch. */
  mtime: number;
}

/** One thing the desktop app met in a drop and did not take, and why. */
export interface NotTaken {
  path: string;
  /**
   * `empty`, `missing`, `not_a_file_or_folder` (a symbolic link most of all,
   * which is never followed), `reserved` (a name coffret keeps for itself),
   * `not_text` or `unreadable` — or, apart from all of them, `system_file` and
   * `system_folder` for a name an operating system keeps (spec: EP-17), which
   * {@link heldBy} takes out into {@link Drop.systemFiles}.
   */
  reason: string;
}

/** What the desktop app answers a drop with. */
export interface HeldDrop {
  /** The drop's one-time handle, or `null` where nothing was taken. */
  handle: string | null;
  files: HeldFile[];
  notTaken: NotTaken[];
}

/** What the desktop app's window offers the explorer's page. */
export interface DesktopBridge {
  /** Hands the dropped files over by their paths, and says what was taken. */
  takeDrop(files: readonly File[]): Promise<HeldDrop>;
}

/** The desktop app's bridge, where the page runs in its window, and `null` in a browser. */
export function desktopBridge(): DesktopBridge | null {
  const bridge = (globalThis as { coffretDesktop?: unknown }).coffretDesktop;
  if (typeof bridge !== 'object' || bridge === null) {
    return null;
  }
  return typeof (bridge as Partial<DesktopBridge>).takeDrop === 'function'
    ? (bridge as DesktopBridge)
    : null;
}

/**
 * A drop the desktop app took nothing of, although it met something with a
 * path in it: the reason nothing was added is what it did not take, which
 * {@link nothingAddedLine} says as such rather than as a drop that could not
 * be read.
 */
export class NothingTaken extends Error {
  readonly notTaken: readonly NotTaken[];

  constructor(notTaken: readonly NotTaken[]) {
    super(notTakenLine(notTaken));
    this.name = 'NothingTaken';
    this.notTaken = notTaken;
  }
}

/**
 * What a drop handed to the desktop app comes to: one record per file it took,
 * each naming the drop's handle and the file's index in what it took, and what
 * it met and did not take.
 *
 * What it did not take is not lost without a word (spec: LA-26). Where it took
 * nothing at all, that is the answer, and it is thrown as {@link NothingTaken};
 * where it took some, it goes beside the files it took, for the confirmation
 * and the notice to say.
 *
 * Except a drop of things that have no path at all — an image dragged out of
 * another page, a selection of text — which the app answers with an empty path
 * for each. That is the drop a browser's walk finds no file in, and it is
 * answered the same way: with no records, so the screen says the drop carried
 * no files rather than that a path named nothing. Where something with a path
 * was not taken beside them, that alone is said.
 *
 * What it left out because an operating system keeps it is not counted among
 * what it did not take: it is handed back apart, also where that is all the
 * drop held, so the screen says so quietly rather than as something that went
 * wrong (spec: EP-17).
 */
export async function heldBy(bridge: DesktopBridge, files: readonly File[]): Promise<Drop> {
  if (files.length === 0) {
    return { files: [], notTaken: [], systemFiles: [] };
  }
  const held = await bridge.takeDrop(files);
  const systemFiles: SystemFile[] = held.notTaken
    .filter((one) => SYSTEM_REASONS.includes(one.reason))
    .map((one) => ({ path: one.path, folder: one.reason === 'system_folder' }));
  const notTaken = held.notTaken.filter(
    (one) => one.reason !== 'empty' && !SYSTEM_REASONS.includes(one.reason),
  );
  const handle = held.handle;
  if (handle === null || held.files.length === 0) {
    if (notTaken.length > 0) {
      throw new NothingTaken(notTaken);
    }
    return { files: [], notTaken: [], systemFiles };
  }
  return {
    files: held.files.map((file, index) => ({
      path: file.path,
      size: file.size,
      mtime: file.mtime,
      source: { kind: 'held', handle, index },
    })),
    notTaken,
    systemFiles,
  };
}

/** The reasons the desktop app gives what an operating system keeps (spec: EP-17). */
const SYSTEM_REASONS: readonly string[] = ['system_file', 'system_folder'];

/**
 * What the confirmation and the notice say of a drop the desktop app took only
 * part of, or `null` where it left nothing out: how many things it did not
 * take, and {@link notTakenLine} of them.
 */
export function notTakenNotice(notTaken: readonly NotTaken[]): string | null {
  if (notTaken.length === 0) {
    return null;
  }
  const things = notTaken.length === 1 ? '1 thing in the drop was' : `${notTaken.length} things in the drop were`;
  return `${things} not taken: ${notTakenLine(notTaken)}`;
}

/**
 * A notice about a drop that was answered without the confirmation, with what
 * the desktop app did not take of it said after it, where it left anything out.
 * After a one-clause line it is the next clause of the same sentence. After
 * lines written as full sentences — a refusal for the drop's budgets, which ends
 * on what to do instead — it is a sentence of its own, so that it does not read
 * as part of that advice.
 */
export function withNotTaken(line: string, notTaken: readonly NotTaken[]): string {
  const left = notTakenNotice(notTaken);
  if (left === null) {
    return line;
  }
  return line.endsWith('.') ? `${line} ${left}.` : `${line}; ${left}`;
}

/**
 * What the screen says of a drop that ended before it had any files: one the
 * desktop app took nothing of says what it did not take, and a browser's walk
 * that was refused says the drop could not be read, and why.
 */
export function nothingAddedLine(cause: unknown): string {
  if (cause instanceof NothingTaken) {
    return `nothing was added — ${notTakenLine(cause.notTaken)}`;
  }
  return `nothing was added — that drop could not be read: ${said(cause)}`;
}

/**
 * What the first thing not taken was, and how many more there were — said to
 * be like it only where every one was not taken for the same reason.
 */
export function notTakenLine(notTaken: readonly NotTaken[]): string {
  const [first] = notTaken;
  const rest = notTaken.length - 1;
  const what = `${first.path === '' ? 'an empty path' : first.path} ${notTakenReason(first.reason)}`;
  if (rest === 0) {
    return what;
  }
  const alike = notTaken.every((one) => one.reason === first.reason);
  return alike ? `${what}, and ${rest} more like it` : `${what}, and ${rest} more not taken`;
}

function notTakenReason(reason: string): string {
  switch (reason) {
    case 'not_a_file_or_folder':
      return 'is a link or something else that is neither a file nor a folder';
    case 'reserved':
      return 'is a name coffret keeps for itself';
    case 'missing':
      return 'is no longer there';
    case 'not_text':
      return 'has a name that is not text';
    case 'empty':
      return 'names nothing';
    default:
      return 'could not be read';
  }
}
