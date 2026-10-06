import { isRefusal, type Refused } from '@coffret/api';

/**
 * Whether a refusal is Storage no longer taking this device's grant: `storage`,
 * with the reason `unauthenticated`. It is the one Storage refusal a retry
 * never clears and a reconnect does (see [`reconnect`](./reconnect)).
 */
export function ranOut(refused: Pick<Refused, 'kind' | 'reason'> | null | undefined): boolean {
  return refused?.kind === 'storage' && refused.reason === 'unauthenticated';
}

/** The same, of something a request threw. */
export function refusedAsRanOut(refused: unknown): boolean {
  return isRefusal(refused) && ranOut(refused);
}
