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
 * A count, its thousands grouped with commas: `12,480`.
 *
 * One spelling on every device, as [`time`](#time) has, rather than the
 * browser's locale: a count read beside a size is read the same way on every
 * desk.
 */
export function grouped(count: number): string {
  return count.toLocaleString('en-US');
}

/**
 * One Entry's modification time, in the reader's own zone, to the minute:
 * `2016-04-11 20:12`.
 *
 * The server states it in UTC because the time belongs to the user's file and
 * means the same thing on every device that opens the Library. Which zone to
 * show it in is the device's, and the browser is the one that knows it — so
 * the wall-clock time is the reader's.
 *
 * To the minute and with no offset, as a file manager's list states it: the
 * column is read at a glance down a list of hundreds, and the seconds and the
 * zone are noise there. One spelling on every device rather than the browser's
 * locale format, which differs between two browsers on one desk.
 *
 * `null` is a count of seconds no calendar reaches, which the server says
 * rather than naming a moment that is not the file's; so does this. A year
 * with no four digits to spell it is stated in UTC the way JavaScript states
 * one, which is the only spelling such a moment has.
 */
export function time(iso: string | null): string {
  if (iso === null) {
    return '—';
  }
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) {
    return '—';
  }
  const year = at.getFullYear();
  if (year < 0 || year > 9999) {
    return at.toISOString();
  }
  const date = `${pad(year, 4)}-${pad(at.getMonth() + 1)}-${pad(at.getDate())}`;
  return `${date} ${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

/** A field of a date or time, zero-padded to `width` digits. */
function pad(value: number, width = 2): string {
  return String(value).padStart(width, '0');
}
