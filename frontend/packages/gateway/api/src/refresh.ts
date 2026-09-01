import { apiUrl, askedForJson } from './request';

/**
 * What one refresh came to.
 *
 * Three numbers and nothing about which files arrived: what changed is the
 * listing's to say, folder by folder, and a screen that was handed a thousand
 * paths would still have to ask the folder it is showing what it now holds.
 */
export interface Refreshed {
  /**
   * Whether the Library had a head this device had not seen.
   *
   * Not the same question as `gained` being zero: a commit that only removed
   * Entries advanced the catalog and gained nothing, and calling that up to date
   * would tell somebody their screen is current when a row has just left it.
   */
  advanced: boolean;
  /**
   * How many current Entries the catalog gained — negative where another
   * device's commit removed more than it added.
   */
  gained: number;
  /** How many current Entries the Library holds now. */
  entries: number;
}

/**
 * Asks the Library what is new — `POST /api/refresh`.
 *
 * The one control on the screen that reaches Storage because somebody pressed
 * it, and the only way this device hears of what another device committed: there
 * is no polling of the remote head, by design. Until a refresh has run, a device
 * that has just joined has an empty Library to show and a running one shows the
 * Library as it was.
 *
 * It brings over the catalog and never the bytes. Every row it adds is `remote`,
 * and opening one is what fetches it, exactly as before.
 */
export function refreshCatalog(signal?: AbortSignal): Promise<Refreshed> {
  return askedForJson<Refreshed>(apiUrl('refresh'), signal, 'POST');
}
