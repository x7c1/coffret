// The explorer's filter: a name typed in the header's search field, the n most
// recently added books, or both, and what the tree is drawn from while one is
// on. Kept free of DOM and of React so it is unit testable.
//
// Which rows a filtered tree keeps is the server's to answer (see
// [`getFilter`](@coffret/api)): out of the whole Library, not just the folders
// this page has listed. This module reads that answer into the shape the tree
// walks — which rows were answered, and which folders stand above them — and
// says what the line over the rows says of it.

import type { FilterAsk, Filtered, FilterRow, FilterSettings } from '@coffret/api';

import { parentOf } from './prospective';
import { ancestry, fileKey, folderKey, keyed, type TreeState } from './tree';

/**
 * What the header's field and its panel are set to: the shape the server keeps
 * them in for this device (see [`useKeptFilter`](./useKeptFilter)).
 */
export type { FilterSettings };

/**
 * How many of the most recent books the panel offers at first — the count the
 * server answers for a device that never kept a filter, too.
 */
const RECENT_AT_FIRST = 10;

/** The field empty and Recent off: no filter at all. */
export const NO_FILTER: FilterSettings = { name: '', recentOn: false, recent: RECENT_AT_FIRST };

/**
 * What the filter is asked with, or `null` where the settings are no filter at
 * all — an empty name with Recent off — and nothing is asked.
 *
 * The name goes as typed: what matches it, case and the width of its
 * characters aside, is the server's to say (spec: LA-40).
 */
export function askOf(settings: FilterSettings): FilterAsk | null {
  const named = settings.name !== '';
  if (!named && !settings.recentOn) {
    return null;
  }
  return {
    ...(named ? { name: settings.name } : {}),
    ...(settings.recentOn ? { recent: settings.recent } : {}),
  };
}

/**
 * The largest count the server keeps for this device (spec: LA-43): a larger
 * one would filter the tree but be refused at every keep, the name's
 * included, for as long as the panel held it.
 */
const MOST_RECENT = 4_294_967_295;

/**
 * The count the panel's number input holds, read: a whole number from 1 to
 * 4,294,967,295, or `null` for anything else — which leaves the count as it
 * was.
 */
export function recentCount(typed: string): number | null {
  if (!/^\s*\d+\s*$/.test(typed)) {
    return null;
  }
  const count = Number(typed.trim());
  return Number.isSafeInteger(count) && count >= 1 && count <= MOST_RECENT ? count : null;
}

/**
 * An answer, read into what the tree walks: the rows answered, by their key,
 * and the folders above them, which are drawn open and dimmed to show where a
 * match stands.
 */
export interface FilterShape {
  /** The rows answered, by their tree key (see [`folderKey`](./tree)). */
  answered: ReadonlyMap<string, FilterRow>;
  /** Every folder above an answered row, the Library root left out: it has no row. */
  above: ReadonlySet<string>;
  /**
   * Whether the Library root is answered as a book (spec: LA-41): the files
   * directly in it are shown, as an answered folder's are.
   */
  rootBook: boolean;
}

export function shapeOf(answer: Filtered): FilterShape {
  const answered = new Map<string, FilterRow>();
  const above = new Set<string>();
  let rootBook = false;
  for (const row of answer.rows) {
    if (row.kind === 'folder' && row.path === '') {
      rootBook = true;
      continue;
    }
    answered.set(row.kind === 'folder' ? folderKey(row.path) : fileKey(row.path), row);
    for (const folder of ancestry(parentOf(row.path))) {
      above.add(folder);
    }
  }
  return { answered, above, rootBook };
}

/**
 * Whether two asks are the same filter: the same name and the same count of
 * recent books, each or both absent alike.
 */
export function sameAsk(one: FilterAsk, other: FilterAsk): boolean {
  return one.name === other.name && one.recent === other.recent;
}

/**
 * The folders open in a tree a filter has just been answered for, on being
 * turned on or on its conditions changing: every one above an answered row,
 * and — where the filter was on already — those of the ones open before that
 * stand inside a folder still answered, so that a matched folder somebody
 * opened stays open as the name is narrowed. A filter answered again for the
 * same ask does not come here (see [`answeredTree`](./filter)).
 */
export function filteredOpen(shape: FilterShape, before: ReadonlySet<string>): Set<string> {
  const open = new Set(shape.above);
  for (const path of before) {
    if (ancestry(path).some((folder) => shape.answered.has(folderKey(folder)))) {
      open.add(path);
    }
  }
  return open;
}

/**
 * The tree once the filter is answered with `shape`, for `ask`, from the one
 * that stood (`held`), where the shape drawn before was answered for `drawn`
 * — `null` where the filter has just been turned on.
 *
 * Turned on, or with its conditions changed (another name, Recent turned on
 * or off, another count), the folders open are [`filteredOpen`](./filter)'s.
 * Answered again for the same ask — the Library changed under it — the
 * folders open are left as they were: none is opened again above an answered
 * row, nor above a row that is answered for the first time, so a person who
 * closed them finds them closed. Either way the focus stays only on a row
 * still drawn (see [`drawnUnder`](./filter)): a page of a matched book the
 * reader is open on stays focused, and so the reader stays open.
 */
export function answeredTree(held: TreeState, shape: FilterShape, ask: FilterAsk, drawn: FilterAsk | null): TreeState {
  if (drawn !== null && sameAsk(ask, drawn)) {
    const focus = held.focus !== null && drawnUnder(shape, held.expanded, held.focus) ? held.focus : null;
    return focus === held.focus ? held : { ...held, focus };
  }
  const expanded = filteredOpen(shape, drawn === null ? new Set() : held.expanded);
  const focus = held.focus !== null && drawnUnder(shape, expanded, held.focus) ? held.focus : null;
  return { focus, expanded };
}

/**
 * Whether the row `key` names is drawn in a tree filtered by `shape` with the
 * folders in `expanded` open: one [`drawnIn`](./filter) draws, or a row inside
 * an answered folder — or directly in a Library root answered as a book —
 * with every folder above it open.
 */
export function drawnUnder(shape: FilterShape, expanded: ReadonlySet<string>, key: string): boolean {
  if (drawnIn(shape, key)) {
    return true;
  }
  const row = keyed(key);
  const folders = ancestry(parentOf(row.path));
  if (!folders.every((folder) => expanded.has(folder))) {
    return false;
  }
  if (folders.length === 0) {
    return shape.rootBook && row.kind === 'file';
  }
  return folders.some((folder) => shape.answered.has(folderKey(folder)));
}

/**
 * Whether the row `key` names is drawn in a tree just filtered by `shape`,
 * before any answered folder has been opened: an answered row, or a folder
 * above one.
 */
export function drawnIn(shape: FilterShape, key: string): boolean {
  if (shape.answered.has(key)) {
    return true;
  }
  const row = keyed(key);
  return row.kind === 'folder' && shape.above.has(row.path);
}

/**
 * The tree once the filter is turned off, from the one it stood in before it
 * was turned on (`normal`) and the filtered one (`filtered`): the folders open
 * before, and the focus left on the row it was on, with the folders above
 * that row opened so that it shows — or, where nothing was focused, the focus
 * as it was before.
 */
export function unfiltered(normal: TreeState, filtered: TreeState): TreeState {
  if (filtered.focus === null) {
    return normal;
  }
  const above = ancestry(parentOf(keyed(filtered.focus).path));
  return { focus: filtered.focus, expanded: new Set([...normal.expanded, ...above]) };
}

/**
 * What the line over a filtered tree says of an answer: how many matched, or
 * that nothing did — and where the answer stops short of every match, how
 * many it shows of how many.
 */
export function matchLine(answer: Filtered): string {
  if (answer.truncated) {
    return (
      `Showing the first ${grouped(answer.rows.length)} of ${grouped(answer.total)} matches` +
      ' — narrow the name to see the rest'
    );
  }
  if (answer.total === 0) {
    return NO_MATCHES;
  }
  return answer.total === 1 ? '1 match' : `${grouped(answer.total)} matches`;
}

/** What the line says of an answer that matched nothing. */
export const NO_MATCHES = 'No matches';

/** What the line says while the first answer for a filter is out. */
export const FILTERING = 'Filtering…';

/** What the line says of a request for the filter that was refused. */
export function filterFailedLine(message: string): string {
  return `the filter could not be applied — ${message}`;
}

/** What the explainer behind the panel's question mark ("About Recent") says of Recent. */
export const RECENT_EXPLAINED =
  'Folders that directly hold files, newest first by when their files were added.';

/** A count with its thousands grouped by commas, whatever the page's locale. */
function grouped(count: number): string {
  return count.toLocaleString('en-US');
}
