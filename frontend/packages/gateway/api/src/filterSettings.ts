import { apiUrl, askedForJson } from './request';

/**
 * What the explorer's tree is filtered by: the header's search field and its
 * More filters panel, as they are set.
 */
export interface FilterSettings {
  /** The name typed, as typed; empty for none. */
  name: string;
  /** Whether the most recent books are asked for. */
  recentOn: boolean;
  /** How many of them, a whole number from 1 to 4,294,967,295; kept while Recent is off. */
  recent: number;
}

/**
 * What this device's explorer last had its tree filtered by —
 * `GET /api/filter-settings`. No filter (an empty name, Recent off, a count of
 * 10) where nothing was kept or what was kept cannot be read (spec: LA-43).
 *
 * Kept by the server as this device's own state, beside the appearance and
 * for the same reason: the desktop app serves the page on a port chosen at
 * each launch, and so on an origin whose storage the next launch does not
 * have. Refused while the Library is locked, since a name says something about
 * what it holds.
 */
export async function getFilterSettings(signal?: AbortSignal): Promise<FilterSettings> {
  return askedForJson<FilterSettings>(apiUrl('filter-settings'), signal);
}

/**
 * Keeps `settings` as what this device's explorer is filtered by —
 * `POST /api/filter-settings` — and answers with them as kept.
 *
 * The body is JSON, sent as a `Blob` that says so, as `setAppearance`'s is.
 */
export async function setFilterSettings(
  settings: FilterSettings,
  signal?: AbortSignal,
): Promise<FilterSettings> {
  const { name, recentOn, recent } = settings;
  const body = new Blob([JSON.stringify({ name, recentOn, recent })], { type: 'application/json' });
  return askedForJson<FilterSettings>(apiUrl('filter-settings'), signal, 'POST', body);
}
