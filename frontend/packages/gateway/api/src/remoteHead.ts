import { apiUrl, askedForJson } from './request';

/**
 * Where the Library's head stands against this device's catalog.
 *
 * `newer` where another device has committed since this one last caught up,
 * and `current` where there is nothing newer. No generations: what a screen
 * does with this is draw the control that catches up as waiting or not.
 */
export type HeadState = 'newer' | 'current';

/** What one look at the remote head came to. */
export interface RemoteHead {
  head: HeadState;
}

/**
 * Asks whether the Library has anything this device has not caught up with —
 * `GET /api/remote-head`.
 *
 * A look and nothing more: the server reads the names Storage lists and no
 * object, so no key is reached for, and the catalog is not moved — that stays
 * [`refreshCatalog`](./refresh)'s to do when somebody presses it. Nor does it
 * keep the Library unlocked: a tab asking on a timer is not a person.
 */
export function getRemoteHead(signal?: AbortSignal): Promise<RemoteHead> {
  return askedForJson<RemoteHead>(apiUrl('remote-head'), signal);
}
