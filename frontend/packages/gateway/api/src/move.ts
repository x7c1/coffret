import type { Refused } from './refusal';
import { refusedOf } from './refusal';
import { apiUrl, askedForJson } from './request';
import type { MovePair, Work } from './work';
import { workOf } from './work';

/**
 * The query a move is named by: `from` and `to` in turn, once per pair, the
 * n-th `to` the destination of the n-th `from`.
 */
function moveUrl(pairs: readonly MovePair[]): string {
  const params: [string, string][] = [];
  for (const pair of pairs) {
    params.push(['from', pair.from]);
    params.push(['to', pair.to]);
  }
  return apiUrl('move', params);
}

/**
 * What moving files and folders would do, worked out before it is asked
 * for — `GET /api/move`.
 *
 * The run's own plan over the Library caught up to its head, so what it says
 * is what {@link moveEntries} of the same pairs does — give or take the Library
 * or this device's folders changing in between, or a rename the disk refuses
 * as crossing filesystems, which leaves the file to leave this device after
 * all.
 */
export interface MovePreview {
  /** The pairs it was asked about, in the order they were named. */
  pairs: MovePair[];
  /** How many files of the Library the move would give a new path; `0` where it is refused. */
  entries: number;
  /** How many of this device's files of them would follow, with one rename. */
  following: number;
  /**
   * How many of them could not follow — no folder this device maps reaches the
   * new path, or the one that does is on another filesystem — and would leave
   * this device: still in the Library, opened again by fetching it.
   */
  leaving: number;
  /** How many bytes those come to on this device. */
  leaving_bytes: number;
  /** This device's files in the move that changed since it placed them. */
  edited: string[];
  /** Files under the move this device has and the Library does not hold yet. */
  uploading: string[];
  /**
   * Every refusal the move would meet, each a `refused_move` in the shape every
   * refusal takes — the set the Library does not admit, an edited file, a file
   * still on its way in. Empty where the move may go ahead.
   */
  refused: Refused[];
}

/**
 * Asks what moving `pairs` would do, without moving anything.
 *
 * A set the move would refuse is answered with its refusals rather than
 * refused, since showing them is what the preview is for. Refused itself for a
 * locked Library, and for a request that names no pair.
 */
export async function previewMove(
  pairs: readonly MovePair[],
  signal?: AbortSignal,
): Promise<MovePreview> {
  const preview = await askedForJson<Omit<MovePreview, 'refused'> & { refused: unknown[] }>(
    moveUrl(pairs),
    signal,
  );
  return { ...preview, refused: preview.refused.map(refusedOf) };
}

/** What asking for a move is answered with: which move it is, and the work answer. */
export interface MoveAccepted {
  /**
   * Which move this is: the pending move's in the work answer while it is
   * pending, and the log entry's once it has ended.
   */
  run: number;
  /** The work answer as it stood once the move was accepted. */
  work: Work;
}

/**
 * Asks for `pairs` to be moved, or renamed, in the Library — `POST /api/move`.
 *
 * What a drag onto a folder, "Rename…" and "Move to…" ask for once the person
 * has seen what {@link previewMove} said and has said yes. The server accepts
 * the move and answers at once, before it is carried out (spec: LA-31): it
 * waits its turn among this device's work, listed in the work answer's
 * `moves`, and then every pair lands in one change to the Library, nothing
 * is uploaded, and this device's files follow — or leave this device where
 * they cannot. What it came to is in the work answer's log, under the same
 * `run`: done, or stopped by a refusal the preview would have shown, asked
 * again once its turn came.
 *
 * Refused before anything is accepted only for a locked Library, and for a
 * set touching the rows of a move still pending — a `refused_move` whose
 * reason is `moving`.
 */
export async function moveEntries(
  pairs: readonly MovePair[],
  signal?: AbortSignal,
): Promise<MoveAccepted> {
  const sent = await askedForJson<{ run: number; work: unknown }>(moveUrl(pairs), signal, 'POST');
  return { run: sent.run, work: workOf(sent.work) };
}
