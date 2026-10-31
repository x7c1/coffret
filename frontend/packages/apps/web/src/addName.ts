// The name a drop is sent under, so that a Stop pressed while its files are
// still being sent can name it (spec: LA-35).

/** How many random bytes a name is drawn from. */
const BYTES = 16;

/**
 * A fresh name for one add: random hex, which the server takes as one — one
 * to sixty-four letters, digits, `-` or `_` — and which names nothing about
 * the files or the folder.
 */
export function addName(): string {
  const drawn = crypto.getRandomValues(new Uint8Array(BYTES));
  return Array.from(drawn, (byte) => byte.toString(16).padStart(2, '0')).join('');
}
