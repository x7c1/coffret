// Packing a folder the Library already holds one file at a time, asked first.
//
// A book added one page at a time — dropped and added one by one, or synced
// from the mapped folder — sits in the Library as a Container per page, and a
// freeze is what puts those pages into Packs (spec: PK-1, PK-2). A folder's
// actions menu offers it as "Pack this folder…": the server is asked what a
// freeze of the folder would pack, the answer is shown, and only a yes arms
// the freeze. Where nothing would be
// packed, the question says why instead of offering Pack — and so it does
// where the folder is being packed already, since Pack would arm nothing, and
// where a file it would pack would make a path both a file and a folder, since
// such a freeze is refused whole and packs nothing (spec: EP-6).
//
// What the answer counts is the freeze's own scan, stopped before a file is
// read, so what is packed is what was counted — give or take files that change
// between the two calls.
//
// Kept free of DOM and of React so it is unit testable, as `packChoice.ts`
// beside it is.

import type { FreezePreview } from '@coffret/api';

import { size } from './humanize';
import { systemCount } from './systemFiles';

/** What the folder's action is called. */
export const PACK_THIS_FOLDER = 'Pack this folder…';

/**
 * Whether a folder offers "Pack this folder…".
 *
 * Only a folder this device maps: an unmapped one has no file here for a freeze
 * to read (spec: EP-9), and the server refuses it. And never the Library root:
 * a freeze of the root's whole folder would be a freeze of the whole Library,
 * which is the command line's run and not a folder's.
 */
export function packOffered(listing: { path: string; mapped: boolean }): boolean {
  return listing.mapped && listing.path !== '';
}

/**
 * What a Pack is, said once in the question: the reason to pack a folder at
 * all, in the terms a person reads a book or an album in.
 */
export const PACK_EXPLAINED =
  'A Pack keeps a folder’s files together, so a book or an album is fetched in one go.';

/** What the confirmation shows, worded for one folder's preview. */
export interface PackQuestion {
  /** The folder the freeze would be armed for. */
  folder: string;
  /** `Pack “BookX”?` */
  title: string;
  /**
   * `80 files, 58.0 MB will be packed`, or where nothing would be, the
   * sentence saying so and why — the folder being packed already among the
   * reasons.
   */
  line: string;
  /** Whether there is anything to pack, which is whether Pack is offered. */
  packs: boolean;
  /**
   * The files that would refuse the freeze, each with what to rename —
   * `notes/x.pdf — adding this file would make "notes" both a file and a
   * folder; …` — and, where the server named only the first few, a last line
   * counting the rest (`and 5 files more like them`). Empty where none would.
   * Where there is any, nothing is packed and Pack is not offered.
   */
  inTheWay: string[];
  /**
   * What would be left as it is, one group a line with why — `2 files already
   * in a Pack`, `3 files not on this device`, `4 files not in the Library yet`
   * — and empty where nothing would.
   */
  leftOut: string[];
  /** Said where a freeze is already running and this one would wait for it. */
  waits: string | null;
}

/** The question for one folder's preview. */
export function questionOf(preview: FreezePreview): PackQuestion {
  const title = `Pack “${preview.folder.split('/').at(-1) ?? preview.folder}”?`;
  // Ahead of the count: a freeze already running or waiting for this folder
  // is the run Pack would ask for, so the press would arm nothing, and
  // promising a run after the current one would be promising one that never
  // comes.
  if (preview.already_packing) {
    return {
      folder: preview.folder,
      title,
      line: 'This folder is being packed already.',
      packs: false,
      inTheWay: [],
      leftOut: [],
      waits: null,
    };
  }
  // Ahead of the count too: a freeze with any such file packs none of the
  // others, so a count of what would be packed would be a promise the run
  // breaks (spec: EP-6).
  if (preview.file_and_folder > 0) {
    return {
      folder: preview.folder,
      title,
      line:
        `Can’t pack this folder: ${wouldMake(preview.file_and_folder)} a path both a file ` +
        'and a folder, so nothing would be packed.',
      packs: false,
      inTheWay: inTheWay(preview),
      leftOut: [],
      waits: null,
    };
  }
  const reasons = leftOutParts(preview);
  if (preview.files === 0) {
    return {
      folder: preview.folder,
      title,
      line: `Nothing to pack: ${nothingBecause(preview, reasons)}.`,
      packs: false,
      inTheWay: [],
      leftOut: [],
      waits: null,
    };
  }
  return {
    folder: preview.folder,
    title,
    line: `${count(preview.files)}, ${size(preview.bytes)} will be packed`,
    packs: true,
    inTheWay: [],
    leftOut: reasons,
    // Worded as the drop's line for the same wait is (`PACKED_AFTER`), so the
    // two ways into a freeze say the queue alike.
    waits: preview.after_current
      ? 'a book is being packed already — this folder is packed after it'
      : null,
  };
}

/**
 * Why a folder with nothing to pack has nothing, in the order a person would
 * look for it: a folder this device cannot reach right now first, since then
 * nothing under it was even looked at.
 */
function nothingBecause(preview: FreezePreview, reasons: string[]): string {
  if (preview.unavailable > 0) {
    return 'the folder chosen for it on this device cannot be reached right now';
  }
  if (reasons.length === 0) {
    return 'it holds no files';
  }
  return reasons.join(', ');
}

/**
 * The broad groups of what a freeze would leave as they are, each with why,
 * where there are any: already in a Pack, in a Pack and changed here (which is
 * `update`'s to carry), not on this device, not in the Library yet, and what
 * the system keeps.
 *
 * Not in the Library yet are the files of a drop still landing, or of one cut
 * off part way, which no freeze selects until that drop has finished receiving
 * (spec: PK-17) — counted whether or not they have reached this device's disk,
 * so that a folder holding nothing else says why there is nothing to pack
 * rather than that it holds no files. Files added here outside such a drop, or
 * whose upload failed, are not among them: a freeze packs those, and `files`
 * counts them.
 */
function leftOutParts(preview: FreezePreview): string[] {
  const parts: string[] = [];
  if (preview.in_pack > 0) {
    parts.push(`${count(preview.in_pack)} already in a Pack`);
  }
  if (preview.changed_in_pack > 0) {
    parts.push(`${count(preview.changed_in_pack)} in a Pack and changed here`);
  }
  if (preview.not_here > 0) {
    parts.push(`${count(preview.not_here)} not on this device`);
  }
  if (preview.receiving > 0) {
    parts.push(`${count(preview.receiving)} not in the Library yet`);
  }
  // What an operating system keeps beside the pages — `.DS_Store`, `Thumbs.db`
  // — which the scan leaves out of every freeze (spec: EP-17).
  const kept = systemCount(preview.system_files, preview.system_folders);
  if (kept !== null) {
    parts.push(kept);
  }
  return parts;
}

/** `1 file would make`, `3 files would each make`. */
function wouldMake(files: number): string {
  return files === 1 ? '1 file would make' : `${files} files would each make`;
}

/**
 * Each file the server named as standing in the way, with its sentence, and
 * how many more there are where it named only the first few.
 */
function inTheWay(preview: FreezePreview): string[] {
  const named = preview.in_the_way.map(({ path, message }) => `${path} — ${message}`);
  const more = preview.file_and_folder - preview.in_the_way.length;
  if (more > 0) {
    named.push(`and ${count(more)} more like them`);
  }
  return named;
}

/** `1 file`, `80 files`. */
function count(files: number): string {
  return files === 1 ? '1 file' : `${files} files`;
}

/** Everything packing one folder reaches out to. */
export interface Packing {
  /** The folder "Pack this folder…" was pressed on. */
  folder: string;
  /** Asks the server what a freeze of the folder would pack. */
  preview: (folder: string) => Promise<FreezePreview>;
  /** Shows the question, and answers whether the person chose Pack. */
  ask: (question: PackQuestion) => Promise<boolean>;
  /** Arms the freeze, and follows it as a drop's freeze is followed. */
  start: (folder: string) => Promise<void>;
  /** Says why the preview or the freeze was refused. */
  refuse: (cause: unknown) => void;
}

/** What one press of "Pack this folder…" came to. */
export type Packed = 'armed' | 'cancelled' | 'refused';

/**
 * Asks what a freeze of the folder would pack, shows it, and arms the freeze
 * only on Pack.
 *
 * A folder with nothing to pack is still shown its question — the sentence
 * saying why — with nothing to press but Close, so the answer to the press is
 * on the screen rather than a dialog that never came up.
 */
export async function askToPack(packing: Packing): Promise<Packed> {
  let preview: FreezePreview;
  try {
    preview = await packing.preview(packing.folder);
  } catch (refused) {
    packing.refuse(refused);
    return 'refused';
  }
  const question = questionOf(preview);
  const chosen = await packing.ask(question);
  if (!chosen || !question.packs) {
    return 'cancelled';
  }
  try {
    await packing.start(question.folder);
  } catch (refused) {
    packing.refuse(refused);
    return 'refused';
  }
  return 'armed';
}
