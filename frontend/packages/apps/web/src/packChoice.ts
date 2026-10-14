// Whether a dropped folder goes into the Library as a Pack, asked before
// anything is sent.
//
// What a drop is added as follows what was dropped. A drop of files is added one
// by one, a Container per file, as it always was. A drop holding a folder may be
// a book — a folder of a few hundred page images, which one by one would be a
// few hundred Storage objects — or a folder of unrelated files somebody wants
// added as they are, and only the person dropping it knows which. So it is
// asked, once the drop has been read and before a byte of it goes: the folder,
// how many files it holds at every depth and how much they come to, and three
// answers. Nothing has been sent when the answer is Cancel, so Cancel needs no
// clean-up.
//
// What the answer "Pack" packs is exactly the files this drop carries: the
// server arms the freeze with the paths the upload wrote (spec: PK-17), so a
// one-file Entry already in a folder of the same name is not drawn in. How many
// Packs that comes to is not said, because it is not the folder's to decide:
// Packs are cut by the freeze's target size (spec: PK-3).
//
// Where a dropped path already holds a file of the Library, the question says
// so by name before anything is sent: which files replace a one-file Entry,
// and how many are refused because a Pack holds them. What the Library holds
// there is read from the listing the explorer already has of the folder the
// drop lands in, and from one listing more for each folder under it that the
// drop also carries — never a whole subtree the drop does not reach.
//
// The same question offers to number the pages and volumes of what was
// dropped (see `renumber.ts`). The answer brings the two options back beside
// the choice, and the drop is renumbered before it is sent either way.
//
// Kept free of DOM and of React so it is unit testable, as `dropped.ts` beside
// it is.

import {
  overdrawnBy,
  type Added,
  type ContainerKind,
  type Listing,
  type Overdrawn,
} from '@coffret/api';

import { size } from './humanize';
import { renumber, type Numbering } from './renumber';

/** One folder a drop carried, and what is in it at every depth. */
export interface DroppedFolder {
  /** Its name, which is the first component of every path under it. */
  name: string;
  /** How many files it holds, however deep. */
  files: number;
  /** How many bytes those files come to. */
  bytes: number;
  /** Whether the Library already has a folder of this name where it lands. */
  exists: boolean;
}

/** What a drop holding at least one folder carries, as the question shows it. */
export interface DropSummary {
  /** The dropped folders, in the order the drop first named them. */
  folders: DroppedFolder[];
  /** How many files were dropped loose beside them. */
  loose: number;
  /** How many files the whole drop carries — every part the request will send. */
  files: number;
}

/** What the person answered. */
export type Choice = 'pack' | 'one_by_one' | 'cancel';

/**
 * The answer as the drop acts on it: nothing at all, or how it is added and
 * which of its names are numbered on the way.
 */
export type Answer = 'cancel' | { choice: Exclude<Choice, 'cancel'>; numbering: Numbering };

/**
 * What the drop holds, or `null` where it holds no folder and nothing is asked.
 *
 * A file's path is relative to the folder it was dropped onto, so a path with a
 * separator is under a dropped folder and its first component is that folder;
 * a path without one was dropped loose. An empty folder carries no file and is
 * not here — there is nothing it could be added as.
 *
 * `existing` is the names of the folders the Library already has where the drop
 * lands, which the question says of a dropped folder of the same name.
 */
export function summarize(files: readonly Added[], existing: readonly string[]): DropSummary | null {
  const folders = new Map<string, DroppedFolder>();
  let loose = 0;
  for (const added of files) {
    const cut = added.path.indexOf('/');
    if (cut === -1) {
      loose += 1;
      continue;
    }
    const name = added.path.slice(0, cut);
    const folder = folders.get(name) ?? {
      name,
      files: 0,
      bytes: 0,
      exists: existing.includes(name),
    };
    folder.files += 1;
    folder.bytes += added.file.size;
    folders.set(name, folder);
  }
  if (folders.size === 0) {
    return null;
  }
  return { folders: [...folders.values()], loose, files: files.length };
}

/**
 * The question itself, which is also the dialog's label. The drop is what is
 * asked about — the answer applies to all of it — so it is the drop that is
 * counted in, folder by folder.
 */
export function questionOf(summary: DropSummary): string {
  const held = summary.folders.length === 1 ? 'a folder' : `${summary.folders.length} folders`;
  return `this drop holds ${held} — how should it go into the Library?`;
}

/** The line naming one dropped folder: `📁 BookX — 80 files, 58.0 MB`. */
export function folderLine(folder: DroppedFolder): string {
  return `📁 ${folder.name} — ${count(folder.files)}, ${size(folder.bytes)}`;
}

/**
 * What the question says of a dropped folder the Library already has, and what
 * adding into it does to what it holds: the files in it stay, and a dropped
 * file whose path one of them already holds replaces that file — or is refused,
 * where a Pack holds it (spec: PK-10). Said here because both answers do it, and
 * Cancel is the one that does not.
 */
export function existsLine(folder: DroppedFolder): string {
  return (
    `${folder.name} already exists here — the files in it stay, and a dropped file ` +
    'with the same name as one of them replaces it (or is refused, where a Pack holds it)'
  );
}

/** What the loose files beside the folders are told, where there are any. */
export function looseLine(summary: DropSummary): string {
  const beside = summary.folders.length === 1 ? 'it' : 'them';
  return `and ${count(summary.loose)} beside ${beside}, added the same way`;
}

/** The three answers, worded for this drop. */
export function choiceLabels(summary: DropSummary): Record<Choice, string> {
  return {
    pack: 'Add as a Pack (recommended)',
    one_by_one:
      summary.files === 1 ? 'Add the 1 file on its own' : `Add the ${summary.files} files one by one`,
    cancel: 'Cancel',
  };
}

/**
 * The files the Library holds where a drop lands, keyed by their path relative
 * to the folder dropped onto, and what holds each: a Container of its own or a
 * Pack. Only Entries are here: a file standing on disk that the Library holds
 * no Entry for is not one the Library has, so a drop over it replaces nothing
 * of the Library's.
 */
export type HeldFiles = ReadonlyMap<string, ContainerKind>;

/** What a drop lands on where nothing is known to be held there. */
export const NOTHING_HELD: HeldFiles = new Map();

/** How many of a drop's files land where the Library already holds one. */
export interface Collisions {
  /** The paths that replace a one-file Entry, in the drop's order. */
  replaced: string[];
  /** The paths a Pack holds, which are refused rather than replaced (spec: PK-10). */
  refused: string[];
}

/** How many names the replacement line spells out before it counts the rest. */
const NAMED_REPLACEMENTS = 5;

/**
 * Which of `files` — under the paths they will be sent as — land on a file the
 * Library holds, and which of those a Pack holds.
 */
export function collisionsOf(files: readonly Added[], held: HeldFiles): Collisions {
  const replaced: string[] = [];
  const refused: string[] = [];
  for (const { path } of files) {
    const kind = held.get(path);
    if (kind === 'one-file') {
      replaced.push(path);
    } else if (kind === 'pack') {
      refused.push(path);
    }
  }
  return { replaced, refused };
}

/**
 * What the question says of the files that replace one-file Entries, naming
 * the first few, or `null` where none does.
 */
export function replacesLine(collisions: Collisions): string | null {
  const { replaced } = collisions;
  if (replaced.length === 0) {
    return null;
  }
  const named = replaced.slice(0, NAMED_REPLACEMENTS).join(', ');
  const more = replaced.length - NAMED_REPLACEMENTS;
  const verb = replaced.length === 1 ? 'replaces the file' : 'replace the files';
  return (
    `${count(replaced.length)} in this drop ${verb} the Library already has at the same ` +
    `path: ${named}${more > 0 ? `, and ${more} more` : ''}`
  );
}

/**
 * What the question says of the files a Pack holds at their path, or `null`
 * where there are none.
 */
export function refusedLine(collisions: Collisions): string | null {
  const { refused } = collisions;
  if (refused.length === 0) {
    return null;
  }
  const verb = refused.length === 1 ? 'is' : 'are';
  return `${count(refused.length)} ${verb} refused, because a Pack holds the file at that path`;
}

/** Every numbering the question can be answered with. */
const EVERY_NUMBERING: readonly Numbering[] = [
  { pages: false, volumes: false },
  { pages: true, volumes: false },
  { pages: false, volumes: true },
  { pages: true, volumes: true },
];

/** `folder` joined to a path relative to it; the Library root is the empty string. */
function under(folder: string, relative: string): string {
  return folder === '' ? relative : `${folder}/${relative}`;
}

/**
 * What the Library holds at the paths a drop may be sent under, read from the
 * listing of the folder it lands in and from more listings of the folders
 * under it that the drop carries.
 *
 * Which paths those are depends on the numbering the question is answered
 * with, so every numbering it can be answered with is asked about: a folder is
 * listed where any of them puts a dropped file under it. That stays within the
 * folders the drop itself names, so a drop onto a large Library lists a few
 * folders rather than walking it.
 */
export async function heldUnder(
  landing: Listing,
  files: readonly Added[],
  list: (folder: string) => Promise<Listing>,
): Promise<HeldFiles> {
  // The folders, relative to the landing folder, that some answer sends a file
  // into.
  const reached = new Set<string>();
  for (const numbering of EVERY_NUMBERING) {
    for (const { path } of renumber(files, numbering).files) {
      for (let cut = path.indexOf('/'); cut !== -1; cut = path.indexOf('/', cut + 1)) {
        reached.add(path.slice(0, cut));
      }
    }
  }
  const held = new Map<string, ContainerKind>();
  const prefix = landing.path === '' ? '' : `${landing.path}/`;
  const read = (listing: Listing): string[] => {
    for (const file of listing.files) {
      if (file.container !== null && file.path.startsWith(prefix)) {
        held.set(file.path.slice(prefix.length), file.container);
      }
    }
    return listing.folders
      .map((folder) => folder.path.slice(prefix.length))
      .filter((relative) => reached.has(relative));
  };
  let next = read(landing);
  while (next.length > 0) {
    const listings = await Promise.all(next.map((relative) => list(under(landing.path, relative))));
    next = listings.flatMap(read);
  }
  return held;
}

/** `1 file`, `80 files`. */
function count(files: number): string {
  return files === 1 ? '1 file' : `${files} files`;
}

/** Everything adding one drop reaches out to, once its files are read. */
export interface Choosing {
  /** Every file the drop carried, each with its path relative to the folder. */
  files: readonly Added[];
  /** The folders the Library already has where the drop lands. */
  existing: readonly string[];
  /**
   * What the Library holds at the paths `files` may be sent under — see
   * {@link heldUnder} — asked once the drop is known to hold a folder
   * and before the question goes up.
   */
  held: (files: readonly Added[]) => Promise<HeldFiles>;
  /**
   * Asks the person what a drop holding a folder is added as, showing what
   * numbering `files` would come to and which of them replace or are refused
   * by what is `held`.
   */
  ask: (summary: DropSummary, files: readonly Added[], held: HeldFiles) => Promise<Answer>;
  /**
   * Sends `files` — the drop's, under the paths they go up as — as a Pack where
   * `freeze` is true and one by one where not.
   */
  send: (files: readonly Added[], freeze: boolean) => Promise<void>;
  /** Says why a drop that cannot be sent at all was not. */
  refuse: (overdrawn: Overdrawn) => void;
}

/** What one drop came to before anything else was said of it. */
export type Chosen = 'sent' | 'refused' | 'cancelled';

/**
 * Adds one drop, asking first where it holds a folder.
 *
 * The budgets come first. A drop that is certain to be refused — a file larger
 * than one part may be, more files than one request may carry — is refused
 * here, without being asked about: asking somebody to choose how to add what
 * cannot be added at all would make the choice the first of two surprises.
 *
 * Then a drop of files alone is sent one by one, as it always was, and one
 * holding a folder is asked about. The answer applies to the whole drop: every
 * folder in it, and the loose files beside them — which keep their names, since
 * they are no folder's pages.
 *
 * The summary counts the files as dropped, and numbering changes none of what
 * it counts: renaming moves no file in or out of a folder it names.
 */
export async function chooseAndAdd(choosing: Choosing): Promise<Chosen> {
  const overdrawn = overdrawnBy(choosing.files);
  if (overdrawn !== null) {
    choosing.refuse(overdrawn);
    return 'refused';
  }
  const summary = summarize(choosing.files, choosing.existing);
  if (summary === null) {
    await choosing.send(choosing.files, false);
    return 'sent';
  }
  const held = await choosing.held(choosing.files);
  const answer = await choosing.ask(summary, choosing.files, held);
  if (answer === 'cancel') {
    return 'cancelled';
  }
  await choosing.send(renumber(choosing.files, answer.numbering).files, answer.choice === 'pack');
  return 'sent';
}
