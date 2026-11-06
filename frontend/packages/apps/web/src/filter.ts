// The explorer's filter: a name typed in the header's search field, the n most
// recently added books, or both, and what the tree is drawn from while one is
// on. Kept free of DOM and of React so it is unit testable.
//
// Which rows a filtered tree keeps is the server's to answer (see
// [`getFilter`](@coffret/api)): out of the whole Library, not just the folders
// this page has listed. This module reads that answer into the shape the tree
// walks — which rows were answered, and which folders stand above them — and
// says what the line over the rows says of it.

import type { FilterAsk, Filtered, FilterRow } from '@coffret/api';

import { parentOf } from './prospective';
import { ancestry, fileKey, folderKey, keyed, type TreeState } from './tree';

/** What the header's field and its panel are set to. */
export interface FilterSettings {
  /** The name typed, as typed. */
  name: string;
  /** Whether the most recent books are asked for. */
  recentOn: boolean;
  /** How many of them, a whole number from 1; kept while Recent is off. */
  recent: number;
}

/** How many of the most recent books the panel offers at first. */
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
 * The count the panel's number input holds, read: a whole number from 1, or
 * `null` for anything else — which leaves the count as it was.
 */
export function recentCount(typed: string): number | null {
  if (!/^\s*\d+\s*$/.test(typed)) {
    return null;
  }
  const count = Number(typed.trim());
  return Number.isSafeInteger(count) && count >= 1 ? count : null;
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
 * The folders open in a tree a filter has just been answered for: every one
 * above an answered row, and — where the filter was on already — those of the
 * ones open before that stand inside a folder still answered, so that a
 * matched folder somebody opened stays open as the name is narrowed.
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

/** What the explanation in the panel says of Recent. */
export const RECENT_EXPLAINED =
  'Folders that directly hold files, newest first by when their files were added.';

/** A count with its thousands grouped by commas, whatever the page's locale. */
function grouped(count: number): string {
  return count.toLocaleString('en-US');
}
