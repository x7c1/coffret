// Taking a file or a folder out of the Library, asked first.
//
// Each folder row and each row of a file the Library holds offers "Delete…"
// in its actions menu, and the rows selected together offer one
// "Delete <n> items…" for all of them: the server is asked what the deletion
// would take out and what it would rebuild, that is shown, and only Delete
// arms it. Cancel arms nothing.
//
// What is shown is what leaves the Library. Once that has committed, the server
// discards this device's own unedited copies of the deleted files into the
// trash and keeps any it finds edited, and the finished line says which, so a
// file the person has just deleted does not go on standing in the folder as one
// the Library does not hold.
//
// A deletion names any number of folders and files: a row's actions menu
// names its own, and the explorer's selection (see [`selection`](./selection)) names
// every row selected, so that a Pack holding several of them is rebuilt once
// rather than once per row. Nothing here differs between one row and many —
// the server takes several at once, and `targetName` words them as a count.
//
// Kept free of DOM and of React so it is unit testable, as `packFolder.ts`
// beside it is.

import {
  isRefusal,
  type Delete,
  type DeletePreview,
  type DeleteTarget,
  type Finding,
  type RefusedPack,
  type RefusalKind,
} from '@coffret/api';

import { size } from './humanize';
import { waitingClause, type WaitsFor } from './turns';

/** What the rows' action is called. */
export const DELETE = 'Delete…';

/** What the action is called for several rows selected together: `Delete 3 items…`. */
export function deleteItems(target: { folders: readonly string[]; paths: readonly string[] }): string {
  return `Delete ${items(namedCount(target))}…`;
}

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
  /** `Delete “albums”?`, or `Delete 3 items?` — the thing the person pressed on. */
  title: string;
  /**
   * `12 files, 340.0 MB will be removed from the Library`, or where nothing
   * would be, the sentence saying so.
   */
  removes: string;
  /**
   * What rebuilding the Packs that keep other files costs, and `null` where no
   * Pack keeps other files: every Container the deletion touches goes whole —
   * a one-file Container, or a Pack whose every file it names
   * (spec: PK-9, PK-10).
   */
  rebuilds: Rebuilds | null;
  /** One sentence per Pack the deletion would be refused for, with its reason. */
  refused: string[];
  /** The named files the Library does not hold, where there are any. */
  missing: string | null;
  /** Said where a deletion is already running and this one would wait for it. */
  waits: string | null;
  /** Whether there is anything to delete, which is whether Delete is offered. */
  deletes: boolean;
}

/**
 * What a Pack rebuild costs, said in the highlighted box of the question:
 * `2 Packs holding other files will be rebuilt`, then `Reads 1.8 GB, writes
 * 1.5 GB`. Rebuilding reads each such Pack whole and uploads its replacement,
 * which is the one part of a deletion that costs time and transfer.
 */
export interface Rebuilds {
  line: string;
  cost: string;
}

/** How many rows a target names: its folders and its files. */
function namedCount(target: { folders: readonly string[]; paths: readonly string[] }): number {
  return target.folders.length + target.paths.length;
}

/**
 * The name a target goes by in a sentence: the one folder's, the one file's,
 * or a count — of files where it names files alone, and of items where it
 * names folders too.
 */
export function targetName(target: { folders: readonly string[]; paths: readonly string[] }): string {
  if (namedCount(target) === 1) {
    return target.folders.length === 1 ? `📁 ${lastOf(target.folders[0])}` : lastOf(target.paths[0]);
  }
  return target.folders.length === 0 ? count(target.paths.length) : items(namedCount(target));
}

/**
 * The question's title: the one row by its name in quotes, `Delete “albums”?`,
 * and several as a count of items, `Delete 3 items?`.
 */
export function titleOf(target: { folders: readonly string[]; paths: readonly string[] }): string {
  if (namedCount(target) === 1) {
    return `Delete “${lastOf(target.folders[0] ?? target.paths[0])}”?`;
  }
  return `Delete ${items(namedCount(target))}?`;
}

/** The question for one preview. */
export function questionOf(preview: DeletePreview): DeleteQuestion {
  const target = { folders: preview.folders, paths: preview.paths };
  const deletes = preview.entries > 0;
  return {
    target,
    title: titleOf(target),
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
}): Rebuilds {
  return {
    line: `${packs(counted.rebuilt)} holding other files will be rebuilt`,
    cost: `Reads ${size(counted.rebuild_read)}, writes ${size(counted.rebuild_written)}`,
  };
}

/** One Pack the deletion is refused for: the files that stay, and why. */
function refusedLine(refused: RefusedPack): string {
  const names = refused.spared.map(lastOf).join(', ');
  const stay = refused.spared.length === 1 ? 'stays' : 'stay';
  return `${names} ${stay} in the Library — ${refused.message}`;
}

/** Everything deleting files and folders reaches out to. */
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
 * What the Activity panel says of a deletion: the sentence, and the target a
 * *try again* beside it would ask about again — `null` where nothing invites
 * one.
 *
 * Only a `conflict` does. Its sentence is the server's "another device changed
 * the Library meanwhile, so nothing was changed — running it again plans from
 * the Library as it is now", and a sentence that says to run it again with
 * nothing to press beside it sends the person back to the row's menu for it.
 * Any other refusal is one asking again would only meet once more.
 */
export interface DeleteNotice {
  text: string;
  retry: DeleteTarget | null;
}

/** The target again, where a refusal of `kind` invites asking about it again. */
function retryOf(target: DeleteTarget, kind: RefusalKind | null): DeleteTarget | null {
  return kind === 'conflict' ? { folders: target.folders, paths: target.paths } : null;
}

/**
 * The notice for a preview or an arming that was refused: what the server said,
 * under the name of what was pressed on. `said` is that sentence, as the
 * explorer words any refusal.
 */
export function refusedNotice(target: DeleteTarget, cause: unknown, said: string): DeleteNotice {
  return {
    text: `${targetName(target)} was not deleted — ${said}`,
    retry: retryOf(target, isRefusal(cause) ? cause.kind : null),
  };
}

/**
 * What the confirmation says where another run holds the turn at this
 * device's pending work: the deletion confirmed now starts after it.
 */
export function startsAfterLine(waits: Exclude<WaitsFor, null>): string {
  return `It starts after the ${waits} under way finishes.`;
}

/**
 * The line a running deletion stands under: what it is deleting and how far
 * it has got.
 *
 * A deletion takes its turn at this device's pending work behind any sync,
 * freeze or deletion armed before it, and one the server says is waiting for
 * its turn says so — in the words [`waitsFor`](./turns.ts) picks for every
 * line, so a line that stays put while a book is packed reads as waiting
 * rather than stuck. It stands in the Activity panel's **In progress** beside
 * the other two flows' lines, ranked as they are.
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
  removing_copies: "discarding this device's copies into the trash",
  fetching: 'reading',
};

/**
 * What a finished deletion is said as: what left the Library, anything it was
 * refused, and what became of this device's own copies — or, for one that
 * stopped, what stopped it, with a *try again* where that was a `conflict`.
 * `null` while it is still running.
 */
export function deletedLine(run: Delete): DeleteNotice | null {
  const name = targetName(run);
  switch (run.status) {
    case 'deleting':
      return null;
    case 'stopped':
      return {
        text: `${name} was not deleted — ${run.stopped.message}`,
        retry: retryOf(run, run.stopped.kind),
      };
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
      parts.push(...findingLines(run.findings));
      return { text: parts.join('; '), retry: null };
    }
  }
}

/**
 * What became of this device's own copies of the deleted files, and anything
 * else the run reported beside them.
 *
 * The discards are counted rather than named: they are what the person asked for,
 * and the line already names what was deleted. A copy kept because it changed
 * is named, because it is the one the person has to do something about; so is
 * anything else — a refused discard, a folder this device could not vouch for, a
 * Keyring repair — said in the server's own sentence, the first of them and
 * how many more, as a sync's findings are.
 */
function findingLines(findings: readonly Finding[]): string[] {
  const discarded = findings.filter((finding) => finding.surfaced === 'Discarded').length;
  const kept = findings
    .filter((finding) => finding.surfaced === 'KeptEdited')
    .map((finding) => lastOf(finding.path ?? ''));
  const others = findings.filter(
    (finding) => finding.surfaced !== 'Discarded' && finding.surfaced !== 'KeptEdited',
  );
  const lines: string[] = [];
  if (discarded > 0) {
    lines.push(`${count(discarded)} discarded into the trash on this device`);
  }
  if (kept.length > 0) {
    lines.push(`kept on this device because ${kept.length === 1 ? 'it' : 'they'} changed: ${kept.join(', ')}`);
  }
  const [first] = others;
  if (first !== undefined) {
    const named = first.path === null ? first.message : `${lastOf(first.path)} — ${first.message}`;
    lines.push(others.length === 1 ? named : `${named} (and ${others.length - 1} more)`);
  }
  return lines;
}

/** `1 file`, `12 files`. */
function count(files: number): string {
  return files === 1 ? '1 file' : `${files} files`;
}

/** `1 item`, `3 items`. */
function items(named: number): string {
  return named === 1 ? '1 item' : `${named} items`;
}

/** `1 Pack`, `2 Packs`. */
function packs(rebuilt: number): string {
  return rebuilt === 1 ? '1 Pack' : `${rebuilt} Packs`;
}

/** The last component of an Entry Path. */
function lastOf(path: string): string {
  return path.split('/').at(-1) ?? path;
}
