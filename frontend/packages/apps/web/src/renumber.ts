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
// - Only an image is a page: a file whose extension is one of `IMAGE`, case
//   ignored. Every other file — `ComicInfo.xml`, `readme.txt` — keeps its
//   name, since readers look such files up by it, and takes no part in the
//   sequence, the width, the order or the check for a number carried twice. A
//   folder with no image is no folder of pages.
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
// - A spread — two numbers of the sequence's frame joined by `+` or `-`, the
//   second written with the frame's leading text or without it — is both
//   numbers padded alike, joined as they were: `p0018+p0019.jpg` and
//   `p0018+0019.jpg` are both `018+019.jpg`. A second number written without
//   the leading text is a spread only where it is the next page, the first
//   plus one: `p0018-2.jpg` is a scanner's second take of page 18, not pages
//   18 to 2, and is put after page 18 as any other name.
// - Every other name is put after the page before it, by taking that page's
//   largest number and `_` in front of it: `020_note.jpg`, `019_x.jpg` after
//   `p18+p19.jpg`, and `004_p4x.jpg` after `p4+p3.jpg`, a spread written last
//   page first as a book read right to left may name it; `+` sorts before
//   `_`, so the name lands after the spread and before the next page. A name
//   before the first page takes the number that page's new name starts with,
//   less one: `000_cover.jpg` before `001.jpg` or `001+000.jpg`, and
//   `001_cover.jpg` before `002+001.jpg`.
// - Where the names put after one page would not sort by bytes in the order
//   they are read in — `cover.jpg` before `Credits.jpg`, `a2` before `a10` —
//   each of them also takes its place among them, between the page's number
//   and the name: `000_1_cover.jpg`, `000_2_Credits.jpg`. Names that sort by
//   bytes already keep the shorter form.
// - Where the new names would not sort in that order after all, or the first
//   page's new name starts with the number `0` and leaves no number for a
//   name before it, the folder's pages are left whole rather than shuffled,
//   and the two names read out of order are said, so the person knows which
//   to rename.
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
  /** Whether images are numbered as pages among their sibling files. */
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
  | { reason: 'same_number' }
  /**
   * A new name would be a sibling's name, or, among pages, the new name of
   * another page.
   */
  | { reason: 'taken' }
  /** Two frames are equally common, and neither is the obvious sequence. */
  | { reason: 'tie' }
  /**
   * Pages whose new names would not sort in the order the old ones are read
   * in, or whose first page's new name starts with the number `0`, `p0` or
   * `p0+p1`, with a name before it: `earlier` is read just before `later`,
   * and numbered they would not stay so. Both are names as dropped, the last
   * component only.
   */
  | { reason: 'order'; earlier: string; later: string };

/** What the preview says about one folder's pages or one parent's volumes. */
export interface Group {
  /** Pages among sibling files, or volumes among sibling folders. */
  kind: 'pages' | 'volumes';
  /** The folder they are in, as the drop names it — the dropped folder first. */
  folder: string;
  /** What is renamed, in the order of the new names. */
  renamed: Rename[];
  /**
   * The names left as they are, both sides relative to the dropped folder, in
   * no order of their own: {@link linesOf} puts them among the renamed in the
   * order they are read in. A name kept in a volume that is renamed still moves
   * with it, so `to` is where it lands rather than always `from`.
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
      kind === 'volumes' ? [...held.folders] : [...held.files].filter(isPage),
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
      unchanged: judged.unchanged.map((name) => ({ from: within(name), to: to(name) })),
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
    // Only images are pages; the other files keep their names, and a page may
    // not take one of them any more than a folder's.
    const pages = [...held.files].filter(isPage);
    if (numbering.pages && pages.length > 0) {
      const others = [...held.files].filter((name) => !isPage(name));
      const judged = judgePages(pages, [...finalFolders, ...others]);
      judged.unchanged.push(...others);
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
    return leftAlone(names, { reason: 'tie' });
  }
  const { members } = sequence;
  if (repeats(members.map((member) => member.value))) {
    return leftAlone(names, { reason: 'same_number' });
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
      return leftAlone(names, { reason: 'taken' });
    }
  }
  return judged(names, renamed);
}

/** One page of the sequence, a single page or a spread. */
interface Page {
  /** The new name. */
  to: string;
  /**
   * The value of the number its new name starts with, the one written first,
   * and of its largest number, which differ only for a spread: `p4+p3` is `4`
   * and `4`, `p3+p4` is `3` and `4`.
   */
  first: string;
  high: string;
}

/**
 * The new names of a group of sibling files: every one renamed, in an order
 * that keeps the order they are read in. `names` are images only; `others`
 * are the final names of the folders beside them and the files that are no
 * pages, which a page may not take.
 */
function judgePages(names: readonly string[], others: readonly string[]): Judged {
  const sequence = sequenceOf(names, splitExtension);
  if (sequence === null) {
    return leftAlone(names, null);
  }
  if (sequence === 'tie') {
    return leftAlone(names, { reason: 'tie' });
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
    return leftAlone(names, { reason: 'same_number' });
  }
  const pad = padding(values);
  const pages = new Map<string, Page>([
    ...sequence.members.map((member): [string, Page] => [
      member.name,
      { to: `${pad(member.value)}${member.kept}`, first: member.value, high: member.value },
    ]),
    ...spreads.map((spread): [string, Page] => [
      spread.name,
      {
        to: `${pad(spread.first)}${spread.joint}${pad(spread.last)}${spread.kept}`,
        first: spread.first,
        high: BigInt(spread.first) < BigInt(spread.last) ? spread.last : spread.first,
      },
    ]),
  ]);

  // Every other name goes after the page before it in the order they are
  // read in, by the largest number of that page. Names before the first page
  // wait for it, and go under the number one less than the one its new name
  // starts with: `001_cover` before `002+001`, `000_cover` before `001+000`.
  const order = [...names].sort(naturally);
  const renamed = new Map<string, string>();
  let run: { value: string | null; names: string[] } = { value: null, names: [] };
  for (const name of order) {
    const page = pages.get(name);
    if (page === undefined) {
      run.names.push(name);
      continue;
    }
    if (run.value === null && run.names.length > 0) {
      if (page.first === '0') {
        return leftAlone(names, { reason: 'order', earlier: run.names[run.names.length - 1], later: name });
      }
      run.value = (BigInt(page.first) - 1n).toString();
    }
    putAfter(run, renamed, pad);
    renamed.set(name, page.to.normalize('NFC'));
    run = { value: page.high, names: [] };
  }
  putAfter(run, renamed, pad);

  // Two files at one path would have the second replace the first as it
  // lands, and a page cannot take the path of a folder or of a file that is
  // no page.
  const finals = order.map((name) => renamed.get(name)!);
  const taken = new Set(others);
  if (repeats(finals) || finals.some((name) => taken.has(name))) {
    return leftAlone(names, { reason: 'taken' });
  }
  // What the names above were built to keep, checked rather than assumed:
  // where a listing would read two of them out of order the folder is left as
  // it was, naming the two. A spread whose numbers are not neighbours does
  // that: after `p2+p4`, `p2x` goes under `004_`, and so after `p3`'s `003`.
  for (let at = 1; at < finals.length; at += 1) {
    if (!readInOrder(finals[at - 1], finals[at])) {
      return leftAlone(names, { reason: 'order', earlier: order[at - 1], later: order[at] });
    }
  }
  return judged(names, renamed);
}

/**
 * The new names of the names read after one page, `run.value` its largest
 * number, set in `renamed`: that number and `_` in front of each name.
 *
 * The prefix keeps a name after its page, but not two names after one page in
 * their order between themselves: `a2` and `a10`, or `cover` and `Credits`,
 * sort the other way by bytes. Where that happens each also takes its place
 * among them, padded alike, between the number and the name: `000_1_cover`,
 * `000_2_Credits`. Where two of them would come out as one name, they keep
 * the shorter form, so that the check for a name taken twice leaves the
 * folder whole.
 */
function putAfter(
  run: { value: string | null; names: readonly string[] },
  renamed: Map<string, string>,
  pad: (value: string) => string,
): void {
  if (run.value === null || run.names.length === 0) {
    return;
  }
  const value = pad(run.value);
  const cut = run.names.map(splitExtension);
  const plain = cut.map(([stem, kept]) => `${value}_${stem}${kept}`.normalize('NFC'));
  const sorts = plain.every((name, at) => at === 0 || readInOrder(plain[at - 1], name));
  const twice = repeats(plain);
  const width = String(run.names.length).length;
  run.names.forEach((name, at) => {
    const [stem, kept] = cut[at];
    const place = String(at + 1).padStart(width, '0');
    renamed.set(
      name,
      sorts || twice ? plain[at] : `${value}_${place}_${stem}${kept}`.normalize('NFC'),
    );
  });
}

/**
 * The two numbers of a spread — two pages scanned as one image — and what
 * joins them, or null where `stem` is not one.
 *
 * A spread is two numbers of the sequence joined by `+` or `-`. The first
 * carries the frame's text around it; the second carries it too, or drops the
 * frame's leading text and keeps only the digits and the trailing text, as
 * books write it as often: `p0018+p0019` and `p0018+0019` alike, or
 * `foobar19_0018-0019` beside `foobar19_0017`, or `0018-0019` where the frame
 * is empty. Only the digits after the frame's leading text are a number, so
 * the `19` of `foobar19_` is never taken for one. Only those shapes are read,
 * since they are the ones books have been seen to use; three pages joined is
 * read as any other name. This is the place to widen when a book needs it.
 *
 * A second number without the leading text — or with none to drop, where the
 * frame has none — is a spread only where it is the next page, the first plus
 * one: `p0018-2` is a scanner's second take of page 18 as often as anything,
 * and read as a spread it would carry 18 twice. One written with the full
 * frame says it is a page of the sequence, and is taken as it is.
 */
function spreadOf(
  stem: string,
  [before, after]: [string, string],
): { first: string; joint: string; last: string } | null {
  const framed = `${escaped(before)}(\\d+)${escaped(after)}`;
  const second = `(${escaped(before)})?(\\d+)${escaped(after)}`;
  const found = new RegExp(`^${framed}([+-])${second}$`).exec(stem);
  if (found === null) {
    return null;
  }
  const first = valueOf(found[1]);
  const last = valueOf(found[4]);
  const bare = before === '' || found[3] === undefined;
  if (bare && BigInt(last) !== BigInt(first) + 1n) {
    return null;
  }
  return { first, joint: found[2], last };
}

/** `text` matched as itself inside a regular expression. */
function escaped(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\/-]/g, '\\$&');
}

/**
 * Two names in the order a file manager shows them: runs of digits compared
 * by value, case ignored, and a name's stem before its extension, so that
 * `p18.jpg` comes before `p18-2.jpg` as `p18` does before `p18-2`. The locale
 * is fixed so that the order does not hang on the machine, and names it calls
 * equal fall back to the Library's order.
 */
function naturally(left: string, right: string): number {
  return (
    NATURAL.compare(splitExtension(left)[0], splitExtension(right)[0]) ||
    NATURAL.compare(left, right) ||
    inLibraryOrder(left, right)
  );
}

const NATURAL = new Intl.Collator('en', { numeric: true, sensitivity: 'base' });

/**
 * Two names of one group in the order the preview lists them: the order they
 * are read in, and among pages the files that are no pages after the pages.
 */
function inReadingOrder(kind: Group['kind'], left: string, right: string): number {
  const late = (name: string) => (kind === 'pages' && !isPage(name) ? 1 : 0);
  return late(left) - late(right) || naturally(left, right);
}

/**
 * The extensions of an image, which alone is a page. Other files beside the
 * pages — `ComicInfo.xml`, `readme.txt` — are looked up by their names.
 */
const IMAGE = new Set(['.jpg', '.jpeg', '.png', '.webp', '.avif', '.gif', '.bmp', '.tif', '.tiff']);

/** Whether a file is a page: an image, by its extension, case ignored. */
function isPage(name: string): boolean {
  return IMAGE.has(splitExtension(name)[1]);
}

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

/** One line of a group in the preview: a rename, or a name left as it is. */
export type Line = { renamed: Rename } | { unchanged: Rename };

/**
 * Every line of a group, renamed and unchanged together, in the order the
 * names are read in — the natural order of the names as dropped — so that a
 * page already named as its number stands among its neighbours rather than
 * at the end. Among pages, the files that are no pages come after them.
 */
export function linesOf(group: Group): Line[] {
  const own = (line: Line) => split('renamed' in line ? line.renamed.from : line.unchanged.from)[1];
  return [
    ...group.renamed.map((rename): Line => ({ renamed: rename })),
    ...group.unchanged.map((kept): Line => ({ unchanged: kept })),
  ].sort((a, b) => inReadingOrder(group.kind, own(a), own(b)));
}

/** Whether showing everything would show more than the preview does now. */
export function hasHidden(groups: readonly Group[]): boolean {
  return groups.some((group) => elided(linesOf(group), false).hidden > 0);
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
  switch (leftWhole.reason) {
    case 'same_number':
      return 'left as they are: two of them carry the same number';
    case 'taken':
      return 'left as they are: a new name is already taken by a name beside them';
    case 'tie':
      return 'left as they are: their names follow more than one pattern equally';
    case 'order':
      return (
        'left as they are: numbering them would change the order they are read in ' +
        `(${leftWhole.earlier} is read before ${leftWhole.later})`
      );
  }
}
