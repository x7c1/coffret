// The files and folders an operating system writes beside a person's own, which
// no drop carries into the Library (spec: EP-17).
//
// `.DS_Store` from the Finder, `Thumbs.db` from Windows Explorer, AppleDouble
// `._` files on a volume without extended attributes, the `__MACOSX` folder a
// Finder-made zip leaves when unpacked — and, at a volume's top, the folders
// the system keeps there. A scan of a mapped folder leaves them out, so a
// browser's drop leaves them out too, on the page and before anything is sent:
// sent, the server would only decline them one by one.
//
// The list's source is `docs/spec/entry-path/system-files.json`. The server
// declares it in `coffret_usecase::system_files` and this file declares it for
// the page; each is held to that file by a test, so neither can drift from it
// alone. Compared byte for byte, case included.
//
// Kept free of DOM, as `drop.ts` beside it is.

/** The names left out wherever they stand. */
export const ANYWHERE: readonly string[] = [
  '.DS_Store',
  // The Finder's custom-folder-icon file: the four letters and a carriage
  // return.
  'Icon\r',
  'Thumbs.db',
  'ehthumbs.db',
  'desktop.ini',
  '.directory',
  '__MACOSX',
];

/** What a name left out wherever it stands begins with: AppleDouble's. */
export const ANYWHERE_PREFIXES: readonly string[] = ['._'];

/**
 * The names left out only directly in a dropped folder — the place a volume's
 * own folders stand when the volume's top is what was dropped.
 */
export const AT_TOP: readonly string[] = [
  '.Spotlight-V100',
  '.fseventsd',
  '.TemporaryItems',
  '$RECYCLE.BIN',
  'System Volume Information',
];

/**
 * Whether a name is one an operating system keeps, `atTop` saying whether it
 * stands directly in a dropped folder (spec: EP-17).
 */
export function isSystemFile(name: string, atTop: boolean): boolean {
  return (
    ANYWHERE.includes(name) ||
    ANYWHERE_PREFIXES.some((prefix) => name.startsWith(prefix)) ||
    (atTop && AT_TOP.includes(name))
  );
}

/** One name a drop left out because an operating system keeps it. */
export interface SystemFile {
  /** Where it stands, relative to the folder the drop was let go of from. */
  path: string;
  /** Whether a folder stood at it, which was not walked. */
  folder: boolean;
}

/** How many distinct names the line spells out before it stops. */
const NAMED = 3;

/**
 * The quiet line a drop says of what it left out because the system keeps it —
 * `left out 3 files the system keeps (.DS_Store, Thumbs.db)` — or `null` where
 * it left none out.
 *
 * Not one of the drop's warnings: such files are in almost every drop, and a
 * warning would read as something gone wrong. The names are the distinct ones,
 * a few at most.
 */
export function systemFilesLine(left: readonly SystemFile[]): string | null {
  const files = left.filter((one) => !one.folder).length;
  const counted = systemCount(files, left.length - files);
  return counted === null ? null : `left out ${counted} (${namesOf(left)})`;
}

/**
 * The same group as "Pack this folder…" names it among what is left as it is —
 * `3 files the system keeps`, `2 files and 1 folder the system keeps` — or
 * `null` where there is none.
 */
export function systemCount(files: number, folders: number): string | null {
  const parts = [
    files > 0 ? (files === 1 ? '1 file' : `${files} files`) : null,
    folders > 0 ? (folders === 1 ? '1 folder' : `${folders} folders`) : null,
  ].filter((part) => part !== null);
  return parts.length === 0 ? null : `${parts.join(' and ')} the system keeps`;
}

/** The distinct last components, the first few of them and how many more. */
function namesOf(left: readonly SystemFile[]): string {
  const names = [...new Set(left.map((one) => shown(one.path.split('/').at(-1) ?? one.path)))];
  const named = names.slice(0, NAMED).join(', ');
  return names.length > NAMED ? `${named}, …` : named;
}

/** A name as a line can hold it: `Icon\r`'s carriage return spelled out. */
function shown(name: string): string {
  return name.replace(/\r/g, '\\r');
}
