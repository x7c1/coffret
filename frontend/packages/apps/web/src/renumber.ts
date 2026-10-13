// Numbering the pages and volumes of a dropped folder, decided before anything
// is sent.
//
// Scanned books arrive named however the scanner or the person behind it chose:
// `king01-#001.jpg … king01-#210.jpg`, `第1巻/ … 第12巻/`, `p-0001.jpg`, with the
// title in the name or not. Once they are in the Library nothing lines them up —
// a listing sorts by bytes — and renaming them afterwards is a change the
// explorer cannot ask for yet. But the name a file goes up under is the path
// the drop puts in the request, so renaming at drop time is a matter of sending
// a different path, and the server needs to know nothing about it.
//
// The rule is deliberately narrow, because a rename somebody did not expect is
// worse than one they did not get:
//
// - A page is judged among the files in the same folder, a volume among the
//   folders under the same parent. The dropped folder's own name is the book or
//   the series, not a volume, and is never changed.
// - The number is the last run of digits in the name (a file's without its
//   extension). Only names sharing the text around that run — its frame — are
//   renamed together, and where siblings fall into several frames only the most
//   populous one is; a tie between two is a folder this cannot read, and is
//   left alone.
// - The new name is the number itself, zero-padded to three digits or to the
//   width of the largest number in the group. The value is kept rather than
//   reassigned, so a gap stays a gap and a volume or page found later lines up
//   with what was numbered before it.
// - A group is left whole where the rename would lose something: two names
//   carrying the same number, or a new name that is already a sibling's.
//
// Kept free of DOM and of React so it is unit testable, as `packChoice.ts`
// beside it is.

import type { Added } from '@coffret/api';

/** Which of the two renames the person asked for. */
export interface Numbering {
  /** Whether files are numbered among their sibling files. */
  pages: boolean;
  /** Whether folders are numbered among their sibling folders. */
  volumes: boolean;
}

/** Neither, which is what a drop is sent with unless somebody asks. */
export const NO_NUMBERING: Numbering = { pages: false, volumes: false };

/** One rename, both sides relative to the dropped folder it is in. */
export interface Rename {
  from: string;
  to: string;
}

/** Why a group that had numbered names was left whole. */
export type LeftWhole =
  /** Two of its names carry the same number, `01` and `1`. */
  | 'same_number'
  /** A new name would be the name of a sibling that is not renamed. */
  | 'taken'
  /** Two frames are equally common, and neither is the obvious sequence. */
  | 'tie';

/** What the preview says about one folder's pages or one parent's volumes. */
export interface Group {
  /** Pages among sibling files, or volumes among sibling folders. */
  kind: 'pages' | 'volumes';
  /** The folder they are in, as the drop names it — the dropped folder first. */
  folder: string;
  /** What is renamed, in the order of the new names. */
  renamed: Rename[];
  /**
   * The names left as they are, sorted, both sides relative to the dropped
   * folder. A name kept in a volume that is renamed still moves with it, so
   * `to` is where it lands rather than always `from`.
   */
  unchanged: Rename[];
  /** Why nothing in it is renamed, where that is not for want of numbers. */
  leftWhole: LeftWhole | null;
}

/** The drop as it will be sent, and what the preview shows of the change. */
export interface Renumbered {
  /** Every file of the drop, in the drop's order, under its path to be sent. */
  files: Added[];
  /** Each folder's groups that were judged, ordered by folder. */
  groups: Group[];
}

/**
 * The drop with its pages and volumes numbered as `numbering` asks.
 *
 * Loose files — dropped beside the folders rather than in one — are no book's
 * pages and are sent as they are. With both options off every path is returned
 * as it came.
 */
export function renumber(files: readonly Added[], numbering: Numbering): Renumbered {
  if (!numbering.pages && !numbering.volumes) {
    return { files: [...files], groups: [] };
  }

  // What each folder of the drop holds directly: its files and its folders,
  // by name. A folder is a separator in a path, so this is read off the paths.
  const folders = new Map<string, { files: Set<string>; folders: Set<string> }>();
  const holding = (folder: string) => {
    let held = folders.get(folder);
    if (held === undefined) {
      held = { files: new Set(), folders: new Set() };
      folders.set(folder, held);
    }
    return held;
  };
  for (const added of files) {
    const parts = added.path.split('/');
    if (parts.length < 2) {
      continue;
    }
    for (let depth = 1; depth < parts.length; depth += 1) {
      const folder = parts.slice(0, depth).join('/');
      const held = holding(folder);
      if (depth === parts.length - 1) {
        held.files.add(parts[depth]);
      } else {
        held.folders.add(parts[depth]);
      }
    }
  }

  // Each folder's new names, keyed by its path as dropped. Volumes are decided
  // before pages so that a page's new name can be checked against the names
  // its sibling folders end up with.
  const volumeNames = new Map<string, Map<string, string>>();
  const pageNames = new Map<string, Map<string, string>>();
  const groups: Group[] = [];
  for (const folder of [...folders.keys()].sort()) {
    const held = folders.get(folder)!;
    let finalFolders = [...held.folders];
    if (numbering.volumes && held.folders.size > 0) {
      const judged = judge([...held.folders], (name) => [name, ''], [...held.files]);
      volumeNames.set(folder, judged.names);
      groups.push(groupOf('volumes', folder, judged));
      finalFolders = finalFolders.map((name) => judged.names.get(name) ?? name);
    }
    if (numbering.pages && held.files.size > 0) {
      const judged = judge([...held.files], splitExtension, finalFolders);
      pageNames.set(folder, judged.names);
      groups.push(groupOf('pages', folder, judged));
    }
  }

  return {
    files: files.map((added) => {
      const to = renamedPath(added.path, volumeNames, pageNames);
      return to === added.path ? added : { ...added, path: to };
    }),
    groups,
  };

  /** One judged group as the preview shows it, paths relative to the drop. */
  function groupOf(kind: Group['kind'], folder: string, judged: Judged): Group {
    const within = (name: string) => relative(`${folder}/${name}`);
    const parentTo = relative(renamedFolder(folder, volumeNames));
    const to = (name: string) => (parentTo === '' ? name : `${parentTo}/${name}`);
    return {
      kind,
      folder,
      renamed: [...judged.names]
        .sort(([, a], [, b]) => (a < b ? -1 : a > b ? 1 : 0))
        .map(([from, name]) => ({ from: within(from), to: to(name) })),
      unchanged: judged.unchanged.sort().map((name) => ({ from: within(name), to: to(name) })),
      leftWhole: judged.leftWhole,
    };
  }
}

/** What one group of sibling names came to. */
interface Judged {
  /** The new name of each name renamed. */
  names: Map<string, string>;
  /** The names left as they are. */
  unchanged: string[];
  leftWhole: LeftWhole | null;
}

/**
 * The new names of a group of siblings.
 *
 * `split` cuts a name into the part the number is read from and what is
 * carried over after the number — a file's extension, lowercased, or nothing
 * for a folder. `others` are the names beside the group that it may not take.
 */
function judge(
  names: readonly string[],
  split: (name: string) => [string, string],
  others: readonly string[],
): Judged {
  const numbered = new Map<string, { name: string; digits: string; kept: string }[]>();
  for (const name of names) {
    const [stem, kept] = split(name);
    const found = NUMBERED.exec(stem);
    if (found === null) {
      continue;
    }
    // The frame is the text either side of the number, kept as a pair so that
    // two frames differing only in where one half ends never read as one.
    const frame = JSON.stringify([found[1] ?? '', found[3]]);
    const members = numbered.get(frame) ?? [];
    members.push({ name, digits: found[2], kept });
    numbered.set(frame, members);
  }

  const leave = (leftWhole: LeftWhole | null): Judged => ({
    names: new Map(),
    unchanged: [...names],
    leftWhole,
  });

  const frames = [...numbered.values()].sort((a, b) => b.length - a.length);
  if (frames.length === 0) {
    return leave(null);
  }
  if (frames.length > 1 && frames[0].length === frames[1].length) {
    return leave('tie');
  }

  // The value is what the digits say, so `007` and `7` are one value; leading
  // zeroes are dropped rather than the digits parsed, which keeps a number of
  // any length exact.
  const members = frames[0].map((member) => ({
    ...member,
    value: member.digits.replace(/^0+(?=\d)/, ''),
  }));
  const values = new Set(members.map((member) => member.value));
  if (values.size !== members.length) {
    return leave('same_number');
  }
  const width = Math.max(WIDTH, ...members.map((member) => member.value.length));
  const renamed = new Map(
    members.map((member) => [
      member.name,
      `${member.value.padStart(width, '0')}${member.kept}`.normalize('NFC'),
    ]),
  );

  // A new name that is already a sibling's would put two files at one path —
  // the second replacing the first as it lands — so the group is left whole.
  // A member keeping its own name (`001.jpg` already is one) takes nothing.
  const kept = new Set([
    ...names.filter((name) => !renamed.has(name)),
    ...others,
  ]);
  for (const [from, to] of renamed) {
    if (to !== from && kept.has(to)) {
      return leave('taken');
    }
  }

  return {
    names: new Map([...renamed].filter(([from, to]) => from !== to)),
    unchanged: names.filter((name) => renamed.get(name) === undefined || renamed.get(name) === name),
    leftWhole: null,
  };
}

/**
 * The last run of digits in a name, and the text before and after it.
 *
 * ASCII digits only: those are what a scanner writes, and a full-width `１`
 * would be read as text and leave the name alone, which is the safe side.
 */
const NUMBERED = /^(.*\D)?(\d+)(\D*)$/;

/** The narrowest a number is written: three digits, which most books fit. */
const WIDTH = 3;

/**
 * A file's name and its extension, the extension lowercased.
 *
 * A leading dot is a hidden file's name rather than an extension, and a name
 * ending in a dot has none.
 */
function splitExtension(name: string): [string, string] {
  const dot = name.lastIndexOf('.');
  if (dot <= 0 || dot === name.length - 1) {
    return [name, ''];
  }
  return [name.slice(0, dot), name.slice(dot).toLowerCase()];
}

/** A path with its first component — the dropped folder — taken off. */
function relative(path: string): string {
  const cut = path.indexOf('/');
  return cut === -1 ? '' : path.slice(cut + 1);
}

/** A folder's path with every volume above and including it renamed. */
function renamedFolder(folder: string, volumeNames: Map<string, Map<string, string>>): string {
  const parts = folder.split('/');
  for (let depth = 1; depth < parts.length; depth += 1) {
    const parent = parts.slice(0, depth).join('/');
    parts[depth] = volumeNames.get(parent)?.get(parts[depth]) ?? parts[depth];
  }
  return parts.join('/');
}

/** A dropped file's path with its folders and its own name renamed. */
function renamedPath(
  path: string,
  volumeNames: Map<string, Map<string, string>>,
  pageNames: Map<string, Map<string, string>>,
): string {
  const cut = path.lastIndexOf('/');
  if (cut === -1) {
    return path;
  }
  const folder = path.slice(0, cut);
  const name = path.slice(cut + 1);
  return `${renamedFolder(folder, volumeNames)}/${pageNames.get(folder)?.get(name) ?? name}`;
}

/**
 * How many renames the preview shows at each end of a group before it says
 * how many are between them. A book's first and last pages are what tell a
 * person the rule read the names the way they meant; the two hundred between
 * them say the same thing again.
 */
const PREVIEW_ENDS = 3;

/** A list as the preview shows it: both ends, and how many were left out. */
export interface Elided<T> {
  head: T[];
  /** How many are between `head` and `tail` and not shown. */
  hidden: number;
  tail: T[];
}

/** `items` cut to `PREVIEW_ENDS` at each end, or all of them where `all`. */
export function elided<T>(items: readonly T[], all: boolean): Elided<T> {
  if (all || items.length <= PREVIEW_ENDS * 2) {
    return { head: [...items], hidden: 0, tail: [] };
  }
  return {
    head: items.slice(0, PREVIEW_ENDS),
    hidden: items.length - PREVIEW_ENDS * 2,
    tail: items.slice(items.length - PREVIEW_ENDS),
  };
}

/** Whether showing everything would show more than the preview does now. */
export function hasHidden(groups: readonly Group[]): boolean {
  return groups.some(
    (group) =>
      elided(group.renamed, false).hidden > 0 || elided(group.unchanged, false).hidden > 0,
  );
}

/** The heading over one group: `Pages in BookX/vol1`, `Volume folders in BookX`. */
export function groupHeading(group: Group): string {
  return `${group.kind === 'pages' ? 'Pages' : 'Volume folders'} in ${group.folder}`;
}

/** One rename as the preview writes it. */
export function renameLine(rename: Rename): string {
  return `${rename.from} → ${rename.to}`;
}

/**
 * One name left as it is, as the preview writes it — with where it lands when
 * its volume is renamed, since "unchanged" alone would say it stays where it
 * was.
 */
export function unchangedLine(kept: Rename): string {
  return kept.from === kept.to
    ? `${kept.from} (unchanged)`
    : `${kept.from} → ${kept.to} (name unchanged)`;
}

/** Why a group was left whole, said to the person who asked for it numbered. */
export function leftWholeLine(leftWhole: LeftWhole): string {
  switch (leftWhole) {
    case 'same_number':
      return 'left as they are: two of them carry the same number';
    case 'taken':
      return 'left as they are: a new name is already taken by a name beside them';
    case 'tie':
      return 'left as they are: their names follow more than one pattern equally';
  }
}
