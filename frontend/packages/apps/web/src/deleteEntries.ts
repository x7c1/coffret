// Taking a file or a folder out of the Library, asked first.
//
// Each folder row and each row of a file the Library holds offers "Delete…":
// the server is asked what the deletion would take out and what it would
// rebuild, that is shown, and only Delete arms it. Cancel arms nothing.
//
// What is shown is what leaves the Library, and nothing about this device's own
// files: a deletion touches no mapped folder, so a file this device holds stays
// where it is.
//
// The explorer has no multi-selection, so a deletion names one file or one
// folder at a time; the server takes several files at once, and nothing here
// would change if a selection named more.
//
// Kept free of DOM and of React so it is unit testable, as `packFolder.ts`
// beside it is.

import type { Delete, DeletePreview, DeleteTarget, RefusedPack } from '@coffret/api';

import { size } from './humanize';
import { waitingClause, type WaitsFor } from './turns';

/** What the rows' action is called. */
export const DELETE = 'Delete…';

/** Said in every question, because it is the one thing a person must know. */
export const NOT_UNDOABLE = 'This cannot be undone from the explorer.';

/**
 * Whether a file row offers "Delete…": only a file the Library holds. A file
 * added on this device and not yet in the Library has nothing to take out of
 * it.
 */
export function deleteOffered(file: { container: string | null }): boolean {
  return file.container !== null;
}

/** What the confirmation shows, worded for one preview. */
export interface DeleteQuestion {
  /** What Delete would arm. */
  target: DeleteTarget;
  /** `Delete 📁 albums?` — the thing the person pressed on. */
  title: string;
  /**
   * `12 files, 340.0 MB will be removed from the Library`, or where nothing
   * would be, the sentence saying so.
   */
  removes: string;
  /**
   * `2 Packs holding other files will be rebuilt — reads 1.8 GB, writes 1.5 GB`,
   * and `null` where no Pack keeps other files.
   */
  rebuilds: string | null;
  /** One sentence per Pack the deletion would be refused for, with its reason. */
  refused: string[];
  /** The named files the Library does not hold, where there are any. */
  missing: string | null;
  /** Said where a deletion is already running and this one would wait for it. */
  waits: string | null;
  /** Whether there is anything to delete, which is whether Delete is offered. */
  deletes: boolean;
}

/** The name a target goes by in a sentence: the folder's or the one file's. */
export function targetName(target: { folder: string | null; paths: readonly string[] }): string {
  if (target.folder !== null) {
    return `📁 ${lastOf(target.folder)}`;
  }
  if (target.paths.length === 1) {
    return lastOf(target.paths[0]);
  }
  return count(target.paths.length);
}

/** The question for one preview. */
export function questionOf(preview: DeletePreview): DeleteQuestion {
  const target = { folder: preview.folder, paths: preview.paths };
  const deletes = preview.entries > 0;
  return {
    target,
    title: `Delete ${targetName(target)}?`,
    removes: deletes
      ? `${count(preview.entries)}, ${size(preview.bytes)} will be removed from the Library`
      : 'nothing will be removed from the Library',
    rebuilds: preview.rebuilt > 0 ? rebuildLine(preview) : null,
    refused: preview.refused.map(refusedLine),
    missing:
      preview.missing.length === 0
        ? null
        : `not in the Library, so nothing to remove: ${preview.missing.map(lastOf).join(', ')}`,
    waits: preview.after_current
      ? 'a deletion is running already — this one runs after it'
      : null,
    deletes,
  };
}

/** What rebuilding the Packs that keep other files costs. */
function rebuildLine(counted: {
  rebuilt: number;
  rebuild_read: number;
  rebuild_written: number;
}): string {
  return (
    `${packs(counted.rebuilt)} holding other files will be rebuilt — ` +
    `reads ${size(counted.rebuild_read)}, ` +
    `writes ${size(counted.rebuild_written)}`
  );
}

/** One Pack the deletion is refused for: the files that stay, and why. */
function refusedLine(refused: RefusedPack): string {
  const names = refused.spared.map(lastOf).join(', ');
  const stay = refused.spared.length === 1 ? 'stays' : 'stay';
  return `${names} ${stay} in the Library — ${refused.message}`;
}

/** Everything deleting one file or folder reaches out to. */
export interface Deleting {
  /** What "Delete…" was pressed on. */
  target: DeleteTarget;
  /** Asks the server what deleting it would do. */
  preview: (target: DeleteTarget) => Promise<DeletePreview>;
  /** Shows the question, and answers whether the person chose Delete. */
  ask: (question: DeleteQuestion) => Promise<boolean>;
  /** Arms the deletion, and follows it. */
  start: (target: DeleteTarget) => Promise<void>;
  /** Says why the preview or the deletion was refused. */
  refuse: (cause: unknown) => void;
}

/** What one press of "Delete…" came to. */
export type Deleted = 'armed' | 'cancelled' | 'refused';

/**
 * Asks what deleting the target would do, shows it, and arms the deletion only
 * on Delete.
 *
 * A target with nothing to delete — every Pack holding it refused — is still
 * shown its question, with nothing to press but Close.
 */
export async function askToDelete(deleting: Deleting): Promise<Deleted> {
  let preview: DeletePreview;
  try {
    preview = await deleting.preview(deleting.target);
  } catch (refused) {
    deleting.refuse(refused);
    return 'refused';
  }
  const question = questionOf(preview);
  const chosen = await deleting.ask(question);
  if (!chosen || !question.deletes) {
    return 'cancelled';
  }
  try {
    await deleting.start(question.target);
  } catch (refused) {
    deleting.refuse(refused);
    return 'refused';
  }
  return 'armed';
}

/**
 * What the confirmation says where a sync or a freeze is under way: the
 * deletion confirmed now starts after it.
 */
export function startsAfterLine(waits: Exclude<WaitsFor, null>): string {
  return `It starts after the ${waits} under way finishes.`;
}

/**
 * The line a running deletion stands under: what it is deleting and how far
 * it has got.
 *
 * A deletion takes its turn at this device's pending work behind any sync,
 * freeze or deletion armed before it, so one with no step yet while another is
 * under way is waiting for it — said, in the words [`waitsFor`](./turns.ts)
 * picks for every line, so a line that stays put while a book is packed reads
 * as waiting rather than stuck.
 */
export function deletingLine(run: Delete, waits: WaitsFor = null): string {
  const name = targetName(run);
  const step = run.step;
  const after = run.waiting > 0 ? ` (${run.waiting} more after it)` : '';
  if (step === null) {
    return waits === null
      ? `deleting ${name}…${after}`
      : `deleting ${name}${waitingClause(waits)}${after}`;
  }
  const counted = step.total === null ? '' : ` ${step.done}/${step.total}`;
  return `deleting ${name} — ${PHASE[step.phase]}${counted}${after}`;
}

/** What each phase of a deletion is called on its line. */
const PHASE: Record<NonNullable<Delete['step']>['phase'], string> = {
  catching_up: 'catching up with the Library',
  settling: 'settling',
  scanning: 'scanning',
  packing: 'rebuilding Packs',
  uploading: 'uploading rebuilt Packs',
  committing: 'committing',
  fetching: 'reading',
};

/**
 * What a finished deletion is said as: what left the Library, and anything it
 * was refused — or, for one that stopped, what stopped it. `null` while it is
 * still running.
 */
export function deletedLine(run: Delete): string | null {
  const name = targetName(run);
  switch (run.status) {
    case 'deleting':
      return null;
    case 'stopped':
      return `${name} was not deleted — ${run.stopped.message}`;
    case 'done': {
      const parts = [
        run.entries === 0
          ? `nothing was removed from the Library for ${name}`
          : `${name} — ${count(run.entries)}, ${size(run.bytes)} removed from the Library`,
      ];
      if (run.rebuilt > 0) {
        parts.push(`${packs(run.rebuilt)} rebuilt`);
      }
      parts.push(...run.refused.map(refusedLine));
      if (run.missing.length > 0) {
        parts.push(`not in the Library: ${run.missing.map(lastOf).join(', ')}`);
      }
      return parts.join('; ');
    }
  }
}

/** `1 file`, `12 files`. */
function count(files: number): string {
  return files === 1 ? '1 file' : `${files} files`;
}

/** `1 Pack`, `2 Packs`. */
function packs(rebuilt: number): string {
  return rebuilt === 1 ? '1 Pack' : `${rebuilt} Packs`;
}

/** The last component of an Entry Path. */
function lastOf(path: string): string {
  return path.split('/').at(-1) ?? path;
}
