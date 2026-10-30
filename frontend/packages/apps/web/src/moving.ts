// Moving and renaming files and folders of the Library: by dragging rows of
// the tree onto a folder, and through "Rename…" and "Move to…" in a row's
// actions menu. Every one of them asks the server's move route for a set of
// `(from, to)` pairs (spec: LA-30, LA-31), which carries them out as one
// rename in the Library (spec: FM-15) and has this device's files follow — or
// leave this device where they cannot (spec: EP-10).
//
// Nothing is written before the person has seen what the server's preview
// said and has said yes. A move the preview refuses — a name the destination
// holds already, a file of this device's that changed since it was placed, a
// file the Library does not hold yet — says why and offers Cancel alone:
// nothing is overwritten or merged.
//
// Kept free of DOM and of React so it is unit testable, as `deleteEntries.ts`
// beside it is.

import {
  componentDefect,
  reservedName,
  type ComponentDefect,
  type Move,
  type MovePair,
  type MovePreview,
} from '@coffret/api';

import { deleteOffered } from './deleteEntries';
import { landingOf } from './destination';
import type { Described } from './details';
import { size } from './humanize';
import { LIBRARY_ROOT } from './location';
import { parentOf, RESERVED } from './prospective';
import { rightClicked, type Picked } from './selection';
import { inLibraryOrder } from './stranded';
import { ancestry, type TreeRow } from './tree';

/** The menu item that renames one file or folder. */
export const RENAME = 'Rename…';

/** The menu item that moves the rows it was opened on into a folder chosen from a list. */
export const MOVE_TO = 'Move to…';

/**
 * Whether a row can be moved: a folder or a file the Library holds. A folder
 * prepared with "New folder…", the folder of a book whose packing has not
 * committed, and a file only added here have nothing in the Library to give a
 * new path.
 */
export function movable(row: Described): boolean {
  return row.kind === 'folder' ? !row.prospective && !row.stranded : deleteOffered(row.file);
}

/**
 * The rows a move of `rows` gives a new path: those that can be moved, less
 * any under a folder among them — the folder moves, and what is under it goes
 * with it.
 */
export function movedOf(rows: readonly Described[]): Described[] {
  const held = rows.filter(movable);
  const folders = held.filter((row) => row.kind === 'folder').map((row) => row.path);
  return held.filter((row) => !folders.some((folder) => row.path.startsWith(`${folder}/`)));
}

/**
 * What grabbing the row `key` drags, and the focus and selection it leaves:
 * the selection, where the row is in one of several, and the row alone
 * otherwise — the rule a right-click follows (see
 * [`rightClicked`](./selection)).
 */
export function grabbed(
  rows: readonly TreeRow[],
  picked: Picked,
  key: string,
): { picked: Picked; moved: Described[] } {
  const { picked: next, target } = rightClicked(rows, picked, key);
  return { picked: next, moved: movedOf(target) };
}

/**
 * Whether moving `moved` into the folder `landing` does anything: not into a
 * folder one of them stands in already, nor into a folder among them or
 * anything under one.
 */
export function movesInto(landing: string, moved: readonly Described[]): boolean {
  return (
    moved.length > 0 &&
    moved.every(
      (row) =>
        parentOf(row.path) !== landing &&
        !(row.kind === 'folder' && (landing === row.path || landing.startsWith(`${row.path}/`))),
    )
  );
}

/**
 * Where a drag of `moved` held over the row `key` lands, or `null` where
 * nothing happens there.
 *
 * A folder row is the destination, and a file row means the folder that file
 * stands in, as a drop of files from outside does (see
 * [`landingOf`](./destination)). Over no row, over a row being dragged, and
 * wherever [`movesInto`](#movesInto) says the move would do nothing, nothing
 * happens: the Library root has no row, and the Library's name in the header
 * takes a move there.
 */
export function moveLandingOver(key: string | null, moved: readonly Described[]): string | null {
  if (key === null || moved.some((row) => row.key === key)) {
    return null;
  }
  const landing = landingOf(key);
  return movesInto(landing, moved) ? landing : null;
}

/** The pairs moving `moved` into `landing` asks for: each under its own name there. */
export function pairsOf(moved: readonly Described[], landing: string): MovePair[] {
  return moved.map((row) => ({ from: row.path, to: under(landing, row.name) }));
}

/** What the pill at the bottom of the tree says while a drag of rows would land in `landing`. */
export function moveLine(landing: string): string {
  return `Move to ${landing === '' ? LIBRARY_ROOT : landing}`;
}

/** What "Move to…" asks first, for the rows called `names`: `Move “Vol. 03” to…`. */
export function moveToTitle(names: readonly string[]): string {
  return `Move ${whatOf(names)} to…`;
}

/** What the confirmation of a move shows, worded for one preview. */
export interface MoveQuestion {
  /** What going ahead asks for. */
  pairs: MovePair[];
  /** `Move “Vol. 03” to “Series A”?`, or `Move 3 items to the Library?`. */
  title: string;
  /** What the move does in the Library, and of this device's files that follow. */
  lines: string[];
  /** How many of this device's files leave it, and their size; `null` where none does. */
  leaving: string | null;
  /** Why the move cannot go ahead, one sentence each; empty where it can. */
  refused: string[];
  /** Whether going ahead is offered: nothing refuses it, and it moves something. */
  moves: boolean;
}

/**
 * The confirmation's title: the one row by its name in quotes, several as a
 * count of items, and the folder they go to by its name — the Library for
 * the root.
 */
export function moveTitle(names: readonly string[], landing: string): string {
  return `Move ${whatOf(names)} to ${whereOf(landing)}?`;
}

/** What the confirmation says for one preview of moving the rows called `names` into `landing`. */
export function moveQuestionOf(preview: MovePreview, names: readonly string[], landing: string): MoveQuestion {
  const refused = refusalsOf(preview);
  const moves = refused.length === 0;
  const lines: string[] = [];
  if (moves) {
    lines.push(`${files(preview.entries)} will be given a new path in the Library — nothing is uploaded`);
    if (preview.following > 0) {
      lines.push(`${files(preview.following)} on this device ${preview.following === 1 ? 'follows' : 'follow'} along`);
    }
  }
  return {
    pairs: preview.pairs,
    title: moveTitle(names, landing),
    lines,
    leaving: moves ? leavingLine(preview) : null,
    refused,
    moves,
  };
}

/**
 * What a preview says will leave this device, or `null` where nothing will:
 * `196 files (1.2 GB) on this device will be removed from it; opening them
 * fetches them again`.
 */
export function leavingLine(preview: { leaving: number; leaving_bytes: number }): string | null {
  if (preview.leaving === 0) {
    return null;
  }
  const them = preview.leaving === 1 ? 'it' : 'them';
  return (
    `${files(preview.leaving)} (${size(preview.leaving_bytes)}) on this device will be removed from it; ` +
    `opening ${them} fetches ${them} again`
  );
}

/**
 * Why a preview refuses the move, in the server's own sentences, each once —
 * or, where it refuses nothing and moves nothing, that there is nothing to
 * move. Empty where the move can go ahead.
 */
function refusalsOf(preview: MovePreview): string[] {
  const said = [...new Set(preview.refused.map((refused) => refused.message))];
  if (said.length === 0 && preview.entries === 0) {
    return [NOTHING_TO_MOVE];
  }
  return said;
}

/** Said where a set moves nothing the Library holds. */
export const NOTHING_TO_MOVE = 'nothing here is in the Library to move';

/** Everything moving files and folders reaches out to. */
export interface Moving {
  /** What is asked for. */
  pairs: MovePair[];
  /** The rows' names, which the question and a refusal name them by. */
  names: string[];
  /** The folder they go to. */
  landing: string;
  /** Asks the server what the move would do. */
  preview: (pairs: MovePair[]) => Promise<MovePreview>;
  /** Shows the question, and answers whether the person chose Move. */
  ask: (question: MoveQuestion) => Promise<boolean>;
  /** Carries it out, answering once it is done. */
  move: (pairs: MovePair[]) => Promise<Move>;
  /** The server answered with what it did. */
  moved: (run: Move) => void;
  /** Says why the preview or the move was refused. */
  refuse: (cause: unknown) => void;
}

/** What one drag, or one "Move to…", came to. */
export type Moved = 'moved' | 'cancelled' | 'refused';

/**
 * Asks what the move would do, shows it, and moves only on Move.
 *
 * A move the preview refuses is still shown its question, with nothing to
 * press but Cancel.
 */
export async function askToMove(moving: Moving): Promise<Moved> {
  let preview: MovePreview;
  try {
    preview = await moving.preview(moving.pairs);
  } catch (refused) {
    moving.refuse(refused);
    return 'refused';
  }
  const question = moveQuestionOf(preview, moving.names, moving.landing);
  const chosen = await moving.ask(question);
  if (!chosen || !question.moves) {
    return 'cancelled';
  }
  let run: Move;
  try {
    run = await moving.move(question.pairs);
  } catch (refused) {
    moving.refuse(refused);
    return 'refused';
  }
  moving.moved(run);
  return 'moved';
}

/**
 * The notice for a move whose preview or request was refused: `“Vol. 03” was
 * not moved — …`, or `3 items were not moved — …`.
 */
export function notMovedLine(names: readonly string[], said: string): string {
  return `${whatOf(names)} ${names.length === 1 ? 'was' : 'were'} not moved — ${said}`;
}

/**
 * Where the path `path` stands once `pairs` have moved: the destination of the
 * pair that names it or a folder above it, and `path` itself where none does.
 */
export function movedPath(path: string, pairs: readonly MovePair[]): string {
  for (const pair of pairs) {
    if (path === pair.from) {
      return pair.to;
    }
    if (path.startsWith(`${pair.from}/`)) {
      return `${pair.to}${path.slice(pair.from.length)}`;
    }
  }
  return path;
}

/** What a name nobody could rename to is said as. */
const SHAPE: Record<ComponentDefect, string> = {
  empty: 'a name is needed',
  separator: 'a name cannot hold a “/”',
  nul: 'a name cannot hold a NUL',
  relative: '“.” and “..” are not names',
};

/**
 * What is wrong with `typed` as the new name of `row`, or `null` where
 * nothing is.
 *
 * The rules are the server's for one component of an Entry Path, less the
 * names coffret keeps for itself (as [`nameDefect`](./prospective) judges a new
 * folder's), and the name of another row in the folder it stands in — a
 * folder or a file of the Library there, or a folder prepared there — since a
 * rename onto one would overwrite it or merge with it, which a move never
 * does. `prospective` is the folders this page draws as not held yet.
 *
 * What was typed is trimmed first, as a new folder's name is. Its own name is
 * no defect: renaming to it is renaming nothing.
 */
export function renameDefect(typed: string, row: Described, prospective: readonly string[]): string | null {
  const name = typed.trim();
  const shape = componentDefect(name);
  if (shape !== null) {
    return SHAPE[shape];
  }
  const reserved = reservedName(name);
  if (reserved !== null) {
    return RESERVED[reserved];
  }
  const path = under(parentOf(row.path), name).normalize('NFC');
  if (path === row.path) {
    return null;
  }
  const listing = row.in;
  const taken =
    listing.folders.some((folder) => folder.path === path) ||
    listing.files.some((file) => file.path === path) ||
    prospective.includes(path);
  return taken ? `there is already something called ${name} here` : null;
}

/** The one pair renaming `row` to `typed` asks for, or `null` where the name is the one it has. */
export function renamePair(row: Described, typed: string): MovePair | null {
  const to = under(parentOf(row.path), typed.trim()).normalize('NFC');
  return to === row.path ? null : { from: row.path, to };
}

/**
 * What a preview of a rename comes to in the dialog: refused for a reason that
 * is not about the name (an edited file, an Entry still uploading), said in
 * the dialog's body and not under the field; files of this device that will
 * leave it, said in the dialog before the rename goes ahead; or nothing in the
 * way.
 */
export type RenameStep = { blocked: string } | { leaving: string } | 'go';

/**
 * The step a rename's preview comes to. `acknowledged` is whether the files
 * leaving were already said for this name, so that a second Rename goes ahead
 * with them.
 */
export function renameStepOf(preview: MovePreview, acknowledged: boolean): RenameStep {
  const said = [...new Set(preview.refused.map((refused) => refused.message))];
  if (said.length > 0) {
    return { blocked: said.join('; ') };
  }
  if (preview.entries === 0) {
    return { blocked: 'nothing here is in the Library to rename' };
  }
  const leaving = leavingLine(preview);
  return leaving !== null && !acknowledged ? { leaving } : 'go';
}

/** One row of the folder tree "Move to…" offers. */
export interface TargetRow {
  /** Its Entry Path; the empty string is the Library root. */
  path: string;
  /** What it is called in the list: its own name, and `Library` for the root. */
  name: string;
  /** How many folders it stands below the root; the root's own is 0. */
  depth: number;
  /** Whether it has folders under it to open; never the root's, which is always open. */
  expandable: boolean;
  /** Whether it is drawn open, the folders under it below it. */
  expanded: boolean;
  /** Whether the move goes there: not where it would do nothing (see {@link movesInto}). */
  takes: boolean;
}

/**
 * The folder tree "Move to…" draws for moving `moved`: a `Library` row at the
 * top for the root, then the folders under each open folder, in the Library's
 * order.
 *
 * Out of `folders`, every folder the Library names and the folders this page
 * draws as not held yet — prepared ones take a move like any other. A folder
 * being moved is drawn, so that the list still reads as the tree, but takes
 * nothing and does not open: nothing under it can.
 */
export function moveTargets(
  folders: readonly string[],
  expanded: ReadonlySet<string>,
  moved: readonly Described[],
): TargetRow[] {
  const all = new Set(folders.flatMap(ancestry));
  const children = new Map<string, string[]>();
  for (const path of [...all].sort(inLibraryOrder)) {
    const parent = parentOf(path);
    children.set(parent, [...(children.get(parent) ?? []), path]);
  }
  const movedFolders = new Set(moved.filter((row) => row.kind === 'folder').map((row) => row.path));
  const rows: TargetRow[] = [
    { path: '', name: LIBRARY_ROOT, depth: 0, expandable: false, expanded: true, takes: movesInto('', moved) },
  ];
  const walk = (parent: string, depth: number) => {
    for (const path of children.get(parent) ?? []) {
      const expandable = (children.get(path)?.length ?? 0) > 0 && !movedFolders.has(path);
      const open = expandable && expanded.has(path);
      rows.push({
        path,
        name: path.slice(path.lastIndexOf('/') + 1),
        depth,
        expandable,
        expanded: open,
        takes: movesInto(path, moved),
      });
      if (open) {
        walk(path, depth + 1);
      }
    }
  };
  walk('', 1);
  return rows;
}

/**
 * The folders "Move to…" opens to: every folder above where the rows stand,
 * so that the place they are moved from is in view to begin with.
 */
export function openedFor(moved: readonly Described[]): Set<string> {
  return new Set(moved.flatMap((row) => ancestry(parentOf(row.path))));
}

/** `parent`'s child called `name`; the empty parent is the Library root. */
function under(parent: string, name: string): string {
  return parent === '' ? name : `${parent}/${name}`;
}

/** The rows a question names: the one by its name in quotes, several as a count. */
function whatOf(names: readonly string[]): string {
  return names.length === 1 ? `“${names[0]}”` : `${names.length} items`;
}

/** The folder a question names: by its own name in quotes, and the root as the Library. */
function whereOf(landing: string): string {
  return landing === '' ? 'the Library' : `“${landing.slice(landing.lastIndexOf('/') + 1)}”`;
}

/** `1 file`, `12 files`. */
function files(count: number): string {
  return count === 1 ? '1 file' : `${count} files`;
}
