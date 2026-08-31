// Turning what a listing row carries into what a column shows. Pure, so the
// rules are stated once and tested rather than repeated per column.

const UNITS = ['B', 'kB', 'MB', 'GB', 'TB', 'PB'] as const;

/**
 * One Entry's plaintext length, in the largest unit that leaves a number a
 * person reads at a glance.
 *
 * Powers of a thousand and not of 1024, because the unit says so: `kB` is a
 * thousand bytes, and a column that showed 1024 of them under that name would
 * be stating a different fact than the one it labels.
 */
export function size(bytes: number): string {
  let scaled = bytes;
  let unit = 0;
  while (scaled >= 1000 && unit < UNITS.length - 1) {
    scaled /= 1000;
    unit += 1;
  }
  // Whole bytes stay whole; everything scaled down keeps one decimal, which is
  // as much precision as the eye uses in a list.
  const shown = unit === 0 ? String(scaled) : scaled.toFixed(1);
  return `${shown} ${UNITS[unit]}`;
}

/**
 * One Entry's modification time, in the reader's own zone.
 *
 * The server states it in UTC because the time belongs to the user's file and
 * means the same thing on every device that opens the Library. Which zone to
 * show it in is the browser's, and the browser is the one that knows the
 * reader's.
 *
 * `null` is a count of seconds no calendar reaches, which the server says
 * rather than naming a moment that is not the file's; so does this.
 */
export function time(iso: string | null): string {
  if (iso === null) {
    return '—';
  }
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) {
    return '—';
  }
  return at.toLocaleString();
}
