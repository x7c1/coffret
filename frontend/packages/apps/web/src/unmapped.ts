// What a gesture is answered with in a folder this device can do nothing in.
//
// The banner over such a folder is the standing reason it is there at all.
// These are the answers to the things that were just tried in front of it, and
// a gesture that changes nothing on the screen needs one: a row clicked and a
// drop let go of both look, a second later, exactly like a screen that was
// never touched.
//
// Two folders reach here and not one: a folder of the Library no mapping of
// this device reaches, and a path the Library holds nothing at. The gestures
// fail in the same place — nothing to fetch, nowhere to put it — so the answer
// is written here for both, and which reason it gives is the whole of what
// `held` decides.
//
// Kept free of DOM and of React so it is unit testable, the way the drop's own
// sentences beside it are.

/** Which gesture met the folder. */
export type Tried = 'open' | 'add';

/**
 * The line the notice area shows, naming what was tried and why it came to
 * nothing.
 *
 * Two sentences rather than one, differing in the half that says what was
 * tried. The reason is the same fact about the same folder, and it is worded the
 * same both times, so that reading the second after the first is reading one
 * new clause rather than a new sentence. What differs is everything a person
 * can check: they clicked a row, or they let go of files, and a line beginning
 * "nothing was added" under a row they clicked would have them looking for the
 * drop they did not make.
 *
 * Which fact it is, is `held`'s to say, and one path never gets both. A folder
 * the Library holds that no mapping of this device reaches is the EP-9 reason:
 * the folder is there, and this device has nowhere to put what is in it. A path
 * the Library holds nothing at is the other, and mapping is not what is wrong
 * with it — the screen over these rows deliberately says nothing about mapping
 * such a path, since being told to map a part of the Library there is none of
 * sends somebody to a terminal for nothing, and a notice that gave the mapping
 * reason anyway would leave one path carrying two explanations at once.
 *
 * `below` is the folders inside this one that this device *does* have a folder
 * for, which is only ever a Library root's children: a mapping stands for one
 * top-level component and everything under it, so deeper down every child
 * repeats its parent's answer. Naming them is the whole of what this adds to
 * the banner. A drop of a nested folder onto an unmapped root is refused entire
 * although a mapped folder below the root could have taken its share, and a
 * refusal that does not say which part was refusable leaves somebody to work
 * out by trial which of their folders the device has. It goes with the EP-9
 * reason alone, and not by being held back: a folder of the Library is what the
 * paths under it imply, so a path the Library does not hold has no children of
 * any kind, mapped or otherwise.
 *
 * Refused entire, and not split. Taking the part that has somewhere to go is
 * the better outcome for a person and is a different gesture underneath: the
 * drop is one request against one folder, so a half-accepted drop is several
 * requests, a rule for which files each carries, and an answer that has to
 * report what landed and what did not from more than one place. What is wrong
 * here is that the refusal says nothing about the folders, and that is wrong
 * under either shape — so it is fixed first, and on its own.
 *
 * It says nothing about `below` for an `open`. The row that was clicked stands
 * in *this* folder, and no folder beside it can hold that file: there is no
 * other place to offer, and offering one would be inviting somebody to look for
 * their file where it is not.
 */
export function unmappedLine(
  tried: Tried,
  held: boolean,
  below: readonly string[],
): string {
  if (!held) {
    // One clause for both gestures, because there is nothing further to say
    // about either: no mapping would change this, and there is no folder below
    // to offer. What was tried is the only half that differs.
    const missing = 'the Library holds nothing at this path';
    return tried === 'open'
      ? `nothing was opened — ${missing}`
      : `nothing was added — ${missing}`;
  }
  const reason = 'no folder on this device holds this part of the Library';
  if (tried === 'open') {
    return `nothing was opened — ${reason}, so there is nowhere here to put the file that row stands for`;
  }
  if (below.length === 0) {
    return `nothing was added — ${reason}`;
  }
  return `nothing was added — ${reason}; the folders below it that do are ${named(below)}, and a drop onto one of those is taken`;
}

/**
 * The folders, as a list a sentence can carry.
 *
 * Two of them and a count of the rest, which is the economy the drop's own
 * refusals keep: the notice area is one line, and a Library root of thirty
 * top-level folders would otherwise put a paragraph in it. Two rather than one
 * because the point of naming any is that a person recognizes theirs, and one
 * name reads as the only answer.
 */
function named(below: readonly string[]): string {
  if (below.length === 1) {
    return below[0];
  }
  if (below.length === 2) {
    return `${below[0]} and ${below[1]}`;
  }
  return `${below[0]}, ${below[1]} and ${below.length - 2} more`;
}
