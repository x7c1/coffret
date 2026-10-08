// What became of one drop, and what the screen does with that.
//
// A drop of many files is one request, and one request has three ends rather
// than two: an answer naming what was written, an answer naming what was
// refused, and no answer at all. The third is the one worth stating here,
// because a drop can break after some of it has landed — [`brokeOff`] below
// says why — leaving those files in the folder with nothing armed to carry
// them in.
//
// From which the one rule this module exists for: the listing is asked again
// whatever the request came to. It is what puts those files on the screen, and
// they are the only evidence the drop left anything behind. A screen that asked
// again only where the request answered leaves somebody looking at the rows
// they had before, being told the server is gone, and dropping the same files a
// second time.
//
// Kept free of DOM and of React so it is unit testable, the way the lock and
// the refresh beside it are.

import { isOverBudget, isRefusal, type Overdrawn, type RefusedPart, type Upload } from '@coffret/api';

import { size } from './humanize';
import { said } from './useAsked';

/** Everything one drop reaches out to once it has been made. */
export interface Dropping {
  /** Asks the server, which is [`addFiles`](@coffret/api). */
  ask: () => Promise<Upload>;
  /** Says what became of the drop, and `null` clears what the last one said. */
  notice: (line: string | null) => void;
  /** Asks the folder on the screen what it holds now. */
  reload: () => void;
  /** Follows the sync or the freeze that would carry what landed in. */
  follow: () => void;
  /**
   * What this device calls the Library, for the command a drop too large for
   * this route is pointed to; `null` while the page does not know it yet.
   */
  library: string | null;
}

/**
 * Adds what was dropped, and puts what came of it on the screen.
 *
 * The reload is unconditional and is the whole point. An answer full of
 * refusals may still carry `written`; an answer refusing the request as a whole
 * arrives after files of it have landed; and a request that broke mid-transfer
 * says nothing at all about which of those two it was. In every one of the
 * three the folder may hold files the rows do not show, and the folder is the
 * one thing that knows.
 *
 * The work answer is followed wherever something may have landed, for the other
 * half of it. What the server arms behind a drop it took whole is a sync or a
 * freeze, and it arms it before it answers — so a request that broke may have
 * broken after the drop landed and the flow carrying it in was already running.
 * This page has not asked for the work answer since it last had a reason to, and
 * where a drop onto a folder is the only thing that happened there is nothing
 * else to start it asking: what is carrying those files into the Library would
 * run with nothing on the screen saying so. Where nothing was armed after all,
 * the first answer that arrives says so and the following stops at it; and
 * where no answer arrives either, the reload beside it is the request somebody
 * is actually waiting on, and that is what says a server has gone.
 *
 * It never rejects. What a refusal becomes is a sentence in the notice area
 * beside the rows.
 */
export async function askToAdd(dropping: Dropping): Promise<void> {
  dropping.notice(null);
  try {
    const upload = await dropping.ask();
    if (upload.refused.length > 0) {
      dropping.notice(refusedLine(upload.refused));
    }
    if (upload.written.length > 0) {
      dropping.follow();
    }
    dropping.reload();
  } catch (refused: unknown) {
    dropping.notice(brokeOff(refused, dropping.library));
    // A drop refused before it was sent armed nothing, so there is nothing to
    // follow; the folder is asked again all the same, as every drop's is.
    if (!isOverBudget(refused)) {
      dropping.follow();
    }
    dropping.reload();
  }
}

/**
 * What an answer's refused parts say, as one line.
 *
 * The first one and a count of the rest. A drop of three hundred pages onto a
 * folder the Library holds a Pack in refuses three hundred times for the one
 * reason, and three hundred sentences would say it three hundred times over a
 * notice area one line high.
 */
export function refusedLine(refused: readonly RefusedPart[]): string {
  const [first] = refused;
  const rest = refused.length - 1;
  return rest === 0
    ? `${first.name} — ${first.message}`
    : `${first.name} — ${first.message} (and ${rest} more)`;
}

/**
 * What a drop that got no answer says.
 *
 * `unreachable` is minted wherever a `fetch` rejects without being aborted, or
 * wherever an answer's status arrived and whose body broke off before it could
 * be read, and for a `GET` its sentence is true: nothing was sent and nothing
 * came back. For an upload it is a guess. The browser was streaming the body,
 * and the server answers every refusal about the cost of a drop — a budget
 * passed, a disk with no room — while it is still streaming, so the very
 * refusals that leave files in the folder are the ones a browser is likeliest
 * to report as a transfer that failed. This screen is the one place that knows
 * which request it made, and so the one place that can decline to pass the
 * guess on.
 *
 * What it says instead claims neither thing it does not know. Not that the
 * server is absent, and not that the files were refused: only that the drop did
 * not finish, and that the folder — which the reload is at that moment asking
 * again — is what says how much of it is there.
 *
 * And it ends on what to do about it, because knowing nothing for certain is
 * where a person is likeliest to be left with no move at all: the rows are what
 * to read, and anything the drop did not leave among them is dropped again.
 * Saying only that the outcome is unknown would be true and would strand
 * somebody who has just lost twenty minutes of sending, which is the same place
 * the sentence this replaces left them.
 *
 * A drop refused before it was sent, for passing a budget of the server's, is
 * said by {@link overBudgetLine}. Every other refusal is the server's own, read
 * off an answer that did arrive, and is shown as it is written.
 */
export function brokeOff(refused: unknown, library: string | null): string {
  if (isOverBudget(refused)) {
    return overBudgetLine(refused.overdrawn, library);
  }
  if (isRefusal(refused) && refused.kind === 'unreachable') {
    return (
      'that drop did not finish sending, so how much of it arrived is not known here — ' +
      'the rows below are the folder as it stands now, and anything not in them can be ' +
      'dropped again'
    );
  }
  return said(refused);
}

/**
 * What a drop refused before it was sent says: which budget it passed, and what
 * carries such a drop into the Library instead.
 *
 * Both halves, because the reason alone strands somebody. Nothing about the
 * Library limits how large a file is, or how many there are (spec: LA-9) — what
 * cannot carry them is this one request. A file copied into the folder on this
 * device that this one is mapped to is carried in by `coffret sync`, whatever
 * its size, so that is the way the sentence points to; where the drop is too
 * many files or too much at once rather than one file too large, dropping fewer
 * files at a time is the other.
 *
 * The Library is named as this device calls it, which is what `--library`
 * takes; a page that does not know the name yet leaves `<library>` in its place,
 * the placeholder the server's own sentences use. A name is only refused for
 * not being one path component, so it may hold a space or a quote, and is
 * quoted where it does — the command is there to be typed or pasted as it stands.
 */
export function overBudgetLine(overdrawn: Overdrawn, library: string | null): string {
  const sync = `\`coffret sync --library ${library === null ? '<library>' : typed(library)}\``;
  // Fewer files at a time, and not subfolders: a drop of loose files has none,
  // and a folder dropped whole may have none either.
  const instead =
    'Drop fewer files at a time, or copy them into this folder on the device it is ' +
    `mapped to and run ${sync}.`;
  switch (overdrawn.budget) {
    case 'part': {
      const [carried, limit] = against(overdrawn.size, overdrawn.limit);
      return (
        `${overdrawn.name} is ${carried}, more than the ${limit} ` +
        'one file can be when dropped here. Copy it into this folder on the device it is ' +
        `mapped to and run ${sync}.`
      );
    }
    case 'parts':
      return (
        `this drop holds ${overdrawn.count} files, more than the ${overdrawn.limit} one drop ` +
        `can carry. ${instead}`
      );
    case 'request': {
      const [carried, limit] = against(overdrawn.carried, overdrawn.limit);
      return `this drop comes to ${carried}, more than the ${limit} one drop can carry. ${instead}`;
    }
  }
}

/**
 * A size past a budget beside that budget, each as the explorer shows a size —
 * unless the two would read the same, as a file a few megabytes over one part
 * does at one decimal of a gigabyte. Then both are given in bytes, so the
 * sentence does not say a size is more than itself.
 */
function against(bytes: number, limit: number): [string, string] {
  if (size(bytes) !== size(limit)) {
    return [size(bytes), size(limit)];
  }
  const exact = (n: number) => `${n.toLocaleString('en-US')} bytes`;
  return [exact(bytes), exact(limit)];
}

/**
 * A Library's name as one argument on a command line: as it is where nothing in
 * it means anything to a shell, and in single quotes where something does.
 */
function typed(name: string): string {
  return /^[\w.@%+=:,/-]+$/.test(name) ? name : `'${name.replaceAll("'", `'\\''`)}'`;
}
