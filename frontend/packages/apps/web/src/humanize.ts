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
 * One Entry's modification time, in the reader's own zone, as RFC 3339 to the
 * second: `2026-10-08T14:03:00+09:00`.
 *
 * The server states it in UTC because the time belongs to the user's file and
 * means the same thing on every device that opens the Library. Which zone to
 * show it in is the device's, and the browser is the one that knows it — so
 * the wall-clock time is the reader's, and the offset beside it says which
 * zone that is, so the text names one moment wherever it is read or pasted.
 *
 * One spelling on every device rather than the browser's locale format, which
 * differs between two browsers on one desk and leaves the zone unsaid. The
 * Entry's time is whole seconds (spec: FM-9), so nothing finer is shown.
 *
 * `null` is a count of seconds no calendar reaches, which the server says
 * rather than naming a moment that is not the file's; so does this. A year
 * RFC 3339 has no four digits for is stated in UTC the way JavaScript states
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
  const clock = `${pad(at.getHours())}:${pad(at.getMinutes())}:${pad(at.getSeconds())}`;
  return `${date}T${clock}${offsetOf(at)}`;
}

/**
 * How far the reader's zone stood from UTC at that moment, as RFC 3339 spells
 * it: `+09:00`, `-04:00`, and `+00:00` for a zone that is UTC.
 *
 * At that moment and not now, because a zone's offset moves with daylight
 * saving and with the zone's own history.
 */
function offsetOf(at: Date): string {
  // `getTimezoneOffset` counts the other way: minutes to add to local time to
  // reach UTC, so a zone east of Greenwich is negative.
  const east = -at.getTimezoneOffset();
  const sign = east < 0 ? '-' : '+';
  const minutes = Math.abs(east);
  return `${sign}${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
}

/** A field of a date or time, zero-padded to `width` digits. */
function pad(value: number, width = 2): string {
  return String(value).padStart(width, '0');
}
