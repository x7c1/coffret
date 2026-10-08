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
// Kept free of DOM and of React so it is unit testable, as `dropped.ts` beside
// it is.

import { overdrawnBy, type Added, type Overdrawn } from '@coffret/api';

import { size } from './humanize';

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
  /** Asks the person what a drop holding a folder is added as. */
  ask: (summary: DropSummary) => Promise<Choice>;
  /** Sends the drop, as a Pack where `freeze` is true and one by one where not. */
  send: (freeze: boolean) => Promise<void>;
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
 * folder in it, and the loose files beside them.
 */
export async function chooseAndAdd(choosing: Choosing): Promise<Chosen> {
  const overdrawn = overdrawnBy(choosing.files);
  if (overdrawn !== null) {
    choosing.refuse(overdrawn);
    return 'refused';
  }
  const summary = summarize(choosing.files, choosing.existing);
  if (summary === null) {
    await choosing.send(false);
    return 'sent';
  }
  const choice = await choosing.ask(summary);
  if (choice === 'cancel') {
    return 'cancelled';
  }
  await choosing.send(choice === 'pack');
  return 'sent';
}
