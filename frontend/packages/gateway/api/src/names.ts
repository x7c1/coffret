/**
 * The rules one name is held to before it can stand as a component of an Entry
 * Path inside a mapped folder: what a page asks of a name somebody typed before
 * anything is sent for it.
 *
 * The server's own rules, said again here rather than learned by sending
 * something and being refused: the shape is the one an Entry Path is parsed to
 * (spec: EP-2), and the reserved names are the ones a scan passes over and a
 * drop is refused for, `reserved` (spec: EP-11, EP-14). A name these pass can
 * still be refused for where it lands — a file standing where the folder would
 * be, a folder no mapping reaches — and that stays the server's to say.
 */

/** Which part of one component's shape a name fails (spec: EP-2). */
export type ComponentDefect = 'empty' | 'separator' | 'nul' | 'relative';

/** What is wrong with `name` as one component of an Entry Path, or `null`. */
export function componentDefect(name: string): ComponentDefect | null {
  if (name === '') {
    return 'empty';
  }
  if (name.includes('/')) {
    return 'separator';
  }
  if (name.includes('\u0000')) {
    return 'nul';
  }
  if (name === '.' || name === '..') {
    return 'relative';
  }
  return null;
}

/** Why a name is coffret's rather than the person's. */
export type ReservedName =
  /** It carries the prefix a half-written file is called by (spec: EP-11). */
  | 'scratch'
  /** It is the device's own management area inside a mapped folder (spec: EP-14). */
  | 'management_area'
  /**
   * It is not the management area's name and folds to it under ASCII case
   * folding, which a case-folding volume would not tell apart (spec: EP-14).
   */
  | 'folds_to_management_area';

/** The folder a mapped root keeps what the device records about it in. */
export const MANAGEMENT_AREA = '.coffret';

/** What every scratch name a local writer fills a file under begins with. */
export const SCRATCH_PREFIX = '.coffret-fetch-';

/**
 * Which of the reserved names `name` is, if any, at any depth inside a mapped
 * folder. The exact names are asked about before the folded spelling, as the
 * server asks them.
 */
export function reservedName(name: string): ReservedName | null {
  if (name.startsWith(SCRATCH_PREFIX)) {
    return 'scratch';
  }
  if (name === MANAGEMENT_AREA) {
    return 'management_area';
  }
  if (asciiLowercase(name) === MANAGEMENT_AREA) {
    return 'folds_to_management_area';
  }
  return null;
}

/** `text` with ASCII letters alone lowercased: the folding the rule names, and no other. */
function asciiLowercase(text: string): string {
  return text.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}
