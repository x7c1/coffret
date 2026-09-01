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

import type { Added } from '@coffret/api';

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

/**
 * Every file under what was dropped, each with its path relative to the folder
 * it was dropped on.
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
export async function filesUnder(entries: DroppedEntry[]): Promise<Added[]> {
  const added: Added[] = [];
  for (const entry of entries) {
    await collect(entry, '', added);
  }
  return added;
}

/** One entry and everything under it, appended to `added`. */
async function collect(entry: DroppedEntry, under: string, added: Added[]): Promise<void> {
  const path = under === '' ? entry.name : `${under}/${entry.name}`;
  if (entry.isFile) {
    added.push({ path, file: await fileOf(entry as DroppedFile) });
    return;
  }
  if (!entry.isDirectory) {
    // Neither a file nor a folder: a browser answers this for a drop that is not
    // a filesystem item at all — a selection of text, an image dragged out of
    // another page. There is nothing to add.
    return;
  }
  for (const child of await childrenOf(entry as DroppedFolder)) {
    await collect(child, path, added);
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
export function droppedFiles(transfer: DataTransfer): Promise<Added[]> {
  const entries: DroppedEntry[] = [];
  for (const item of Array.from(transfer.items)) {
    const entry = item.webkitGetAsEntry();
    if (entry !== null) {
      entries.push(entry);
    }
  }
  return filesUnder(entries);
}
