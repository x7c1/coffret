import { apiUrl, askedForJson } from './request';

/**
 * Which set of colours this device's explorer is drawn in: the one the system
 * asks for, or one a person chose over it.
 */
export type Appearance = 'system' | 'light' | 'dark';

/** What the server answers about the appearance, read or kept. */
interface Answered {
  appearance: Appearance;
}

/**
 * The set this device's explorer is drawn in, as it was last chosen here —
 * `GET /api/appearance`. `system` where nothing was chosen.
 *
 * Kept by the server as this device's own state rather than in the page's
 * storage: the desktop app serves the page on a port chosen at each launch,
 * and so on an origin whose storage the next launch does not have.
 */
export async function getAppearance(signal?: AbortSignal): Promise<Appearance> {
  return (await askedForJson<Answered>(apiUrl('appearance'), signal)).appearance;
}

/**
 * Keeps `appearance` as this device's choice — `POST /api/appearance` — and
 * answers with it as kept.
 *
 * The body is JSON, sent as a `Blob` that says so, as `mapFolder`'s is.
 */
export async function setAppearance(appearance: Appearance, signal?: AbortSignal): Promise<Appearance> {
  const body = new Blob([JSON.stringify({ appearance })], { type: 'application/json' });
  return (await askedForJson<Answered>(apiUrl('appearance'), signal, 'POST', body)).appearance;
}
