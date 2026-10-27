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
// Two rules, one for each option, because a book's pages and its volumes come
// to the drop in different shapes.
//
// What both share:
//
// - A page is judged among the files in the same folder, a volume among the
//   folders under the same parent. The dropped folder's own name is the book or
//   the series, not a volume, and is never changed.
// - The number is the last run of digits in the name (a file's without its
//   extension). Names sharing the text around that run — its frame — are one
//   sequence, and where siblings fall into several frames the most populous one
//   is the sequence; a tie between two is a folder this cannot read, and is left
//   alone.
// - The new name is the number itself, zero-padded to three digits or to the
//   width of the largest number in the group. The value is kept rather than
//   reassigned, so a gap stays a gap and a volume or page found later lines up
//   with what was numbered before it.
// - A group is left whole where the rename would lose something: two names
//   carrying the same number, or a new name that is already a sibling's.
//
// Volumes keep a deliberately narrow rule, because a rename somebody did not
// expect is worse than one they did not get: only the sequence is renamed, and
// every other folder keeps its name.
//
// Pages are read in order, and the order is the one a file manager shows: the
// natural order of their names, digits compared by value. A rename that kept
// the numbered pages and left `cover.jpg` or a joined spread `p0018+p0019.jpg`
// as it was would send both to the end of the book, since a listing sorts by
// bytes and digits sort before letters. So every page is renamed, and the new
// names sort by bytes in the order the old ones did naturally:
//
// - A spread — two numbers of the sequence's frame joined by `+` or `-` — is
//   both numbers padded alike, joined as they were: `018+019.jpg`.
// - Every other name is put after the page before it, by taking that page's
//   last number and `_` in front of it: `020_note.jpg`, `019_x.jpg` after a
//   spread, and `000_cover.jpg` for a name before the first page.
// - Where the new names would not sort in that order after all, or the first
//   page is `0` and leaves no number for a name before it, the folder's pages
//   are left whole rather than shuffled.
//
// Neither rule can read every book the way its owner meant, so the person may
// correct a new name before anything is sent: the last component of a name the
// rule renamed, never the folder it lands in. A correction is kept as data
// beside the rule — the dropped path it corrects and the name it gives — and
// `renumber` lays it over the rule's names, so a volume's corrected name moves
// the pages under it as the rule's name would have. The rule itself is decided
// as if nothing were corrected, which keeps the name it gave to put back.
//
// - A name that cannot be held beside its siblings is refused: an empty one,
//   one with `/` in it, `.` and `..`, and one another new or kept name in the
//   same folder already is — a file's among the files and folders beside it, a
//   folder's likewise.
// - A name that breaks the order its siblings are read in is taken all the
//   same, since it may be what was meant, and said, since it may be a typo.
//   The order is the one the page rule keeps: the byte order of the final
//   names against the natural order of the dropped ones, among pages and among
//   volumes alike. Only an order the correction breaks is said, not one the
//   rule already left as it was among the volumes it does not rename.
//
// Kept free of DOM and of React so it is unit testable, as `packChoice.ts`
// beside it is.

import type { Added } from '@coffret/api';

import { inLibraryOrder } from './stranded';

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
  /**
   * Where the person corrected the new name: the name the rule gave it, the
   * last component only, which putting it back restores.
   */
  ruled?: string;
  /** Where a corrected name breaks the order its siblings are read in. */
  misordered?: Misordered;
}

/**
 * How a corrected name breaks the order: it sorts before the name read just
 * before it, or after the one read just after it — `neighbour`, as that name
 * ends up.
 */
export interface Misordered {
  sorts: 'before' | 'after';
  neighbour: string;
}

/**
 * The person's corrections of the rule's new names, keyed by the path as
 * dropped of what each corrects — a file's for a page, a folder's for a volume
 * — and holding the last component it goes up under instead.
 */
export type Edits = ReadonlyMap<string, string>;

/** No correction, which is how every drop starts. */
export const NO_EDITS: Edits = new Map();

/** Why a group that had numbered names was left whole. */
export type LeftWhole =
  /** Two of its names carry the same number, `01` and `1`. */
  | 'same_number'
  /**
   * A new name would be a sibling's name, or, among pages, the new name of
   * another page.
   */
  | 'taken'
  /** Two frames are equally common, and neither is the obvious sequence. */
  | 'tie'
  /**
   * Pages whose new names would not sort in the order the old ones are read
   * in, or whose first number is `0` with a name before it.
   */
  | 'order';

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
 * The drop with its pages and volumes numbered as `numbering` asks, and the
 * new names in `edits` taken in place of the rule's.
 *
 * Loose files — dropped beside the folders rather than in one — are no book's
 * pages and are sent as they are. With both options off every path is returned
 * as it came. A correction of a name the rule does not rename is no correction
 * the preview offers, and is passed over.
 */
export function renumber(
  files: readonly Added[],
  numbering: Numbering,
  edits: Edits = NO_EDITS,
): Renumbered {
  if (!numbering.pages && !numbering.volumes) {
    return { files: [...files], groups: [] };
  }
  const { folders, judgedGroups, named } = numbered(files, numbering, edits);
  const groups = judgedGroups.map(({ kind, folder, judged }) =>
    groupOf(kind, folder, judged, (kind === 'volumes' ? named.volumes : named.pages).get(folder)!),
  );
  return {
    files: files.map((added) => {
      const to = renamedPath(added.path, named.volumes, named.pages);
      return to === added.path ? added : { ...added, path: to };
    }),
    groups,
  };

  /**
   * One judged group as the preview shows it, paths relative to the drop and
   * `final` its names as corrected. The renames are in the order of the rule's
   * names, so a corrected line stays where the person found it.
   */
  function groupOf(
    kind: Group['kind'],
    folder: string,
    judged: Judged,
    final: ReadonlyMap<string, string>,
  ): Group {
    const within = (name: string) => relative(`${folder}/${name}`);
    const parentTo = relative(renamedFolder(folder, named.volumes));
    const to = (name: string) => (parentTo === '' ? name : `${parentTo}/${name}`);
    const held = folders.get(folder)!;
    const misordered = misorderedIn(
      kind === 'volumes' ? [...held.folders] : [...held.files],
      judged.names,
      final,
    );
    return {
      kind,
      folder,
      renamed: [...judged.names]
        .sort(([, a], [, b]) => inLibraryOrder(a, b))
        .map(([from, name]) => {
          const as = final.get(from) ?? name;
          const rename: Rename = { from: within(from), to: to(as) };
          if (as !== name) {
            rename.ruled = name;
          }
          const broken = misordered.get(from);
          if (broken !== undefined) {
            rename.misordered = broken;
          }
          return rename;
        }),
      unchanged: judged.unchanged.sort().map((name) => ({ from: within(name), to: to(name) })),
      leftWhole: judged.leftWhole,
    };
  }
}

/** What each folder of a drop holds directly: its files and folders, by name. */
type Holdings = Map<string, { files: Set<string>; folders: Set<string> }>;

/** Each folder's new names, keyed by its path as dropped. */
type NamesIn = Map<string, Map<string, string>>;

/** A drop numbered: what was judged by the rule, and the names with `edits` laid over. */
interface Numbered {
  folders: Holdings;
  /** Each judged group, ordered by folder, volumes before the pages beside them. */
  judgedGroups: { kind: Group['kind']; folder: string; judged: Judged }[];
  /** The rule's names, of volumes and of pages. */
  rule: { volumes: NamesIn; pages: NamesIn };
  /** The rule's names with the corrections in `edits` in place of theirs. */
  named: { volumes: NamesIn; pages: NamesIn };
}

/** The work behind {@link renumber}, kept apart so a correction can be checked against it. */
function numbered(files: readonly Added[], numbering: Numbering, edits: Edits): Numbered {
  // What each folder of the drop holds directly: its files and its folders,
  // by name. A folder is a separator in a path, so this is read off the paths.
  const folders: Holdings = new Map();
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
  const volumeNames: NamesIn = new Map();
  const pageNames: NamesIn = new Map();
  const judgedGroups: Numbered['judgedGroups'] = [];
  for (const folder of [...folders.keys()].sort()) {
    const held = folders.get(folder)!;
    let finalFolders = [...held.folders];
    if (numbering.volumes && held.folders.size > 0) {
      const judged = judgeVolumes([...held.folders], [...held.files]);
      volumeNames.set(folder, judged.names);
      judgedGroups.push({ kind: 'volumes', folder, judged });
      finalFolders = finalFolders.map((name) => judged.names.get(name) ?? name);
    }
    if (numbering.pages && held.files.size > 0) {
      const judged = judgePages([...held.files], finalFolders);
      pageNames.set(folder, judged.names);
      judgedGroups.push({ kind: 'pages', folder, judged });
    }
  }

  return {
    folders,
    judgedGroups,
    rule: { volumes: volumeNames, pages: pageNames },
    named: { volumes: overlaid(volumeNames, edits), pages: overlaid(pageNames, edits) },
  };
}

/** `names` with each correction in `edits` of a name in it taken in place of the rule's. */
function overlaid(names: NamesIn, edits: Edits): NamesIn {
  const laid: NamesIn = new Map([...names].map(([folder, within]) => [folder, new Map(within)]));
  for (const [path, name] of edits) {
    const [folder, own] = split(path);
    const within = laid.get(folder);
    if (within?.has(own)) {
      within.set(own, name);
    }
  }
  return laid;
}

/** A dropped path cut into the folder it is in and its own name. */
function split(path: string): [string, string] {
  const cut = path.lastIndexOf('/');
  return cut === -1 ? ['', path] : [path.slice(0, cut), path.slice(cut + 1)];
}

/**
 * Which corrected names of one group of siblings break the order they are
 * read in, and how — judged among all of `siblings`, as dropped, with `rule`
 * the rule's new names and `final` those names as corrected. A pair the
 * rule's names already read out of order is the rule's to answer for, not the
 * correction's, and is not said.
 */
function misorderedIn(
  siblings: readonly string[],
  rule: ReadonlyMap<string, string>,
  final: ReadonlyMap<string, string>,
): Map<string, Misordered> {
  const said = new Map<string, Misordered>();
  const order = [...siblings].sort(naturally);
  const ruled = order.map((name) => rule.get(name) ?? name);
  const finals = order.map((name) => final.get(name) ?? name);
  const breaks = (at: number) =>
    !readInOrder(finals[at - 1], finals[at]) && readInOrder(ruled[at - 1], ruled[at]);
  order.forEach((name, at) => {
    if (finals[at] === ruled[at]) {
      return;
    }
    if (at > 0 && breaks(at)) {
      said.set(name, { sorts: 'before', neighbour: finals[at - 1] });
    } else if (at < order.length - 1 && breaks(at + 1)) {
      said.set(name, { sorts: 'after', neighbour: finals[at + 1] });
    }
  });
  return said;
}

/**
 * Whether a name read before another still comes first in a listing, which
 * sorts by bytes. Every check of the order the pages are read in asks this.
 */
function readInOrder(earlier: string, later: string): boolean {
  return inLibraryOrder(earlier, later) < 0;
}

/** What one group of sibling names came to. */
interface Judged {
  /** The new name of each name renamed. */
  names: Map<string, string>;
  /** The names left as they are. */
  unchanged: string[];
  leftWhole: LeftWhole | null;
}

/** The names of a group sharing the frame its sequence is read from. */
interface Sequence {
  /** The text before the number, and after it up to the extension. */
  frame: [string, string];
  /** Each name of the frame, its number's value and what it carries over. */
  members: { name: string; value: string; kept: string }[];
}

/**
 * The sequence of a group of siblings: the names in its most populous frame.
 *
 * `split` cuts a name into the part the number is read from and what is
 * carried over after the number — a file's extension, lowercased, or nothing
 * for a folder. Null where no name carries a number, and `'tie'` where two
 * frames are equally common.
 */
function sequenceOf(
  names: readonly string[],
  split: (name: string) => [string, string],
): Sequence | 'tie' | null {
  const frames = new Map<string, Sequence>();
  for (const name of names) {
    const [stem, kept] = split(name);
    const found = NUMBERED.exec(stem);
    if (found === null) {
      continue;
    }
    // The frame is the text either side of the number, kept as a pair so that
    // two frames differing only in where one half ends never read as one.
    const frame: [string, string] = [found[1] ?? '', found[3]];
    const key = JSON.stringify(frame);
    const sequence = frames.get(key) ?? { frame, members: [] };
    sequence.members.push({ name, value: valueOf(found[2]), kept });
    frames.set(key, sequence);
  }
  const sorted = [...frames.values()].sort((a, b) => b.members.length - a.members.length);
  if (sorted.length === 0) {
    return null;
  }
  if (sorted.length > 1 && sorted[0].members.length === sorted[1].members.length) {
    return 'tie';
  }
  return sorted[0];
}

/**
 * The value a run of digits says, so that `007` and `7` are one value.
 *
 * Leading zeroes are dropped rather than the digits parsed, which keeps a
 * number of any length exact.
 */
function valueOf(digits: string): string {
  return digits.replace(/^0+(?=\d)/, '');
}

/** Whether any value is carried twice. */
function repeats(values: readonly string[]): boolean {
  return new Set(values).size !== values.length;
}

/** A group with nothing in it renamed, and why where there is a reason. */
function leftAlone(names: readonly string[], leftWhole: LeftWhole | null): Judged {
  return { names: new Map(), unchanged: [...names], leftWhole };
}

/** The new names of `renamed`, leaving out those that keep their own. */
function judged(names: readonly string[], renamed: Map<string, string>): Judged {
  return {
    names: new Map([...renamed].filter(([from, to]) => from !== to)),
    unchanged: names.filter((name) => (renamed.get(name) ?? name) === name),
    leftWhole: null,
  };
}

/**
 * The new names of a group of sibling folders: the narrow rule, under which
 * only the folders of the sequence are renamed. `others` are the files beside
 * them, whose names a folder may not take.
 */
function judgeVolumes(names: readonly string[], others: readonly string[]): Judged {
  const sequence = sequenceOf(names, (name) => [name, '']);
  if (sequence === null) {
    return leftAlone(names, null);
  }
  if (sequence === 'tie') {
    return leftAlone(names, 'tie');
  }
  const { members } = sequence;
  if (repeats(members.map((member) => member.value))) {
    return leftAlone(names, 'same_number');
  }
  const pad = padding(members.map((member) => member.value));
  const renamed = new Map(
    members.map((member) => [member.name, `${pad(member.value)}${member.kept}`.normalize('NFC')]),
  );

  // A new name that is already a sibling's would put two folders at one path,
  // so the group is left whole. A member keeping its own name (`001` already
  // is one) takes nothing.
  const kept = new Set([...names.filter((name) => !renamed.has(name)), ...others]);
  for (const [from, to] of renamed) {
    if (to !== from && kept.has(to)) {
      return leftAlone(names, 'taken');
    }
  }
  return judged(names, renamed);
}

/** One page of the sequence, a single page or a spread. */
interface Page {
  /** The new name. */
  to: string;
  /** The value of its first number, and of its last. */
  first: string;
  last: string;
}

/**
 * The new names of a group of sibling files: every one renamed, in an order
 * that keeps the order they are read in. `others` are the final names of the
 * folders beside them, which a file may not take.
 */
function judgePages(names: readonly string[], others: readonly string[]): Judged {
  const sequence = sequenceOf(names, splitExtension);
  if (sequence === null) {
    return leftAlone(names, null);
  }
  if (sequence === 'tie') {
    return leftAlone(names, 'tie');
  }

  const members = new Set(sequence.members.map((member) => member.name));
  const spreads = names.flatMap((name) => {
    if (members.has(name)) {
      return [];
    }
    const [stem, kept] = splitExtension(name);
    const spread = spreadOf(stem, sequence.frame);
    return spread === null ? [] : [{ name, kept, ...spread }];
  });

  // A spread's numbers are pages too: `p0019` beside `p0018+p0019` is a page
  // carried twice, and the widest of them sets the width of all.
  const values = [
    ...sequence.members.map((member) => member.value),
    ...spreads.flatMap((spread) => [spread.first, spread.last]),
  ];
  if (repeats(values)) {
    return leftAlone(names, 'same_number');
  }
  const pad = padding(values);
  const pages = new Map<string, Page>([
    ...sequence.members.map((member): [string, Page] => [
      member.name,
      { to: `${pad(member.value)}${member.kept}`, first: member.value, last: member.value },
    ]),
    ...spreads.map((spread): [string, Page] => [
      spread.name,
      {
        to: `${pad(spread.first)}${spread.joint}${pad(spread.last)}${spread.kept}`,
        first: spread.first,
        last: spread.last,
      },
    ]),
  ]);

  // Every other name goes after the page before it in the order they are
  // read in, by the last number of that page. Names before the first page
  // wait for it, and go under the number one less than its first.
  const order = [...names].sort(naturally);
  const renamed = new Map<string, string>();
  const after = (value: string, name: string) => {
    const [stem, kept] = splitExtension(name);
    return `${pad(value)}_${stem}${kept}`;
  };
  let before: string | null = null;
  const waiting: string[] = [];
  for (const name of order) {
    const page = pages.get(name);
    if (page === undefined) {
      if (before === null) {
        waiting.push(name);
      } else {
        renamed.set(name, after(before, name));
      }
      continue;
    }
    if (before === null && waiting.length > 0) {
      if (page.first === '0') {
        return leftAlone(names, 'order');
      }
      const previous = (BigInt(page.first) - 1n).toString();
      for (const waited of waiting) {
        renamed.set(waited, after(previous, waited));
      }
    }
    renamed.set(name, page.to);
    before = page.last;
  }
  for (const [from, to] of renamed) {
    renamed.set(from, to.normalize('NFC'));
  }

  // Two files at one path would have the second replace the first as it
  // lands, and a file cannot share its path with a folder.
  const finals = order.map((name) => renamed.get(name)!);
  const folders = new Set(others);
  if (repeats(finals) || finals.some((name) => folders.has(name))) {
    return leftAlone(names, 'taken');
  }
  // The prefix keeps a name after its page, but not two names after one page
  // in their order between themselves: `a2` and `a10` sort the other way by
  // bytes. Where that happens the folder is left as it was.
  for (let at = 1; at < finals.length; at += 1) {
    if (!readInOrder(finals[at - 1], finals[at])) {
      return leftAlone(names, 'order');
    }
  }
  return judged(names, renamed);
}

/**
 * The two numbers of a spread — two pages scanned as one image — and what
 * joins them, or null where `stem` is not one.
 *
 * A spread is two numbers of the sequence, each with the frame's text around
 * it, joined by `+` or `-`: `p0018+p0019`, or `0018-0019` where the frame is
 * empty. Only that shape is read, since it is the one books have been seen to
 * use; three pages joined, or a second number without the frame, is read as
 * any other name. This is the place to widen when a book needs it.
 */
function spreadOf(
  stem: string,
  [before, after]: [string, string],
): { first: string; joint: string; last: string } | null {
  const numbered = `${escaped(before)}(\\d+)${escaped(after)}`;
  const found = new RegExp(`^${numbered}([+-])${numbered}$`).exec(stem);
  if (found === null) {
    return null;
  }
  return { first: valueOf(found[1]), joint: found[2], last: valueOf(found[3]) };
}

/** `text` matched as itself inside a regular expression. */
function escaped(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\/-]/g, '\\$&');
}

/**
 * Two names in the order a file manager shows them: runs of digits compared
 * by value, case ignored. The locale is fixed so that the order does not hang
 * on the machine, and names it calls equal fall back to the Library's order.
 */
function naturally(left: string, right: string): number {
  return NATURAL.compare(left, right) || inLibraryOrder(left, right);
}

const NATURAL = new Intl.Collator('en', { numeric: true, sensitivity: 'base' });

/** A value padded to the width of the widest of `values`, three at least. */
function padding(values: readonly string[]): (value: string) => string {
  const width = Math.max(WIDTH, ...values.map((value) => value.length));
  return (value) => value.padStart(width, '0');
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
function renamedFolder(folder: string, volumeNames: NamesIn): string {
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
  volumeNames: NamesIn,
  pageNames: NamesIn,
): string {
  if (!path.includes('/')) {
    return path;
  }
  const [folder, name] = split(path);
  return `${renamedFolder(folder, volumeNames)}/${pageNames.get(folder)?.get(name) ?? name}`;
}

/** Why a corrected name cannot be taken. */
export type Refusal =
  /** It is empty. */
  | 'empty'
  /** It has `/` in it, which would say where the file lands rather than its name. */
  | 'separator'
  /** It is `.` or `..`, which no Entry Path holds (spec: EP-2). */
  | 'dots'
  /** Another new or kept name in the same folder already is it. */
  | 'taken';

/** The dropped path a line of `group` corrects, which {@link Edits} are keyed by. */
export function editedPath(group: Group, rename: Rename): string {
  return `${group.folder}/${split(rename.from)[1]}`;
}

/**
 * `edits` with the new name of the dropped `path` corrected to `name`, or put
 * back to the rule's where `name` is null — or why `name` cannot be taken.
 *
 * The name is checked against what the drop comes to with every other
 * correction in place: a page's against the final names of the files and the
 * folders beside it, a volume's likewise, since a file and a folder cannot
 * share a path either. A name the rule gave is no correction, and leaves none.
 */
export function corrected(
  files: readonly Added[],
  numbering: Numbering,
  edits: Edits,
  path: string,
  name: string | null,
): { edits: Edits } | { refused: Refusal } {
  const others = new Map(edits);
  others.delete(path);
  if (name === null) {
    return { edits: others };
  }
  const as = name.normalize('NFC');
  if (as === '') {
    return { refused: 'empty' };
  }
  if (as.includes('/')) {
    return { refused: 'separator' };
  }
  if (as === '.' || as === '..') {
    return { refused: 'dots' };
  }
  const [folder, own] = split(path);
  const { folders, rule, named } = numbered(files, numbering, others);
  const held = folders.get(folder);
  const volume = held?.folders.has(own) ?? false;
  const ruled = (volume ? rule.volumes : rule.pages).get(folder)?.get(own);
  if (held === undefined || ruled === undefined) {
    // Nothing the rule renames, so no line the preview offers to correct.
    return { edits: others };
  }
  const beside = [
    ...[...held.files]
      .filter((sibling) => volume || sibling !== own)
      .map((sibling) => named.pages.get(folder)?.get(sibling) ?? sibling),
    ...[...held.folders]
      .filter((sibling) => !volume || sibling !== own)
      .map((sibling) => named.volumes.get(folder)?.get(sibling) ?? sibling),
  ];
  if (beside.includes(as)) {
    return { refused: 'taken' };
  }
  if (as !== ruled) {
    others.set(path, as);
  }
  return { edits: others };
}

/** Why a corrected name was not taken, said on its line. */
export function refusalLine(refusal: Refusal): string {
  switch (refusal) {
    case 'empty':
      return 'a name cannot be empty';
    case 'separator':
      return 'a name cannot contain /: only the name is corrected here, not where it lands';
    case 'dots':
      return 'a name cannot be . or ..';
    case 'taken':
      return 'there is already something with this name here';
  }
}

/** What a corrected line says of the order it breaks, since it may be a typo. */
export function misorderedLine(misordered: Misordered): string {
  return `read out of order: this name sorts ${misordered.sorts} ${misordered.neighbour}`;
}

/** The mark on a line whose new name the person corrected. */
export const EDITED = '(edited)';

/** What puts back the name the rule gave. */
export const PUT_BACK = 'put back';

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
    case 'order':
      return 'left as they are: numbering them would change the order they are read in';
  }
}
