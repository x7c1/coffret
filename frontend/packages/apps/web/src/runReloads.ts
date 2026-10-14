import { useEffect } from 'react';

import type { Freeze, Sync } from '@coffret/api';

/**
 * Asks for the open folder's listing and for the folder tree again as the sync
 * and the freeze move.
 *
 * The rows an added file has are the folder's own answer about it, and the
 * moment the sync commits they become ordinary Entry rows. Which is why this
 * watches the status and not the count — a sync reports what it did when it
 * has done it. And a commit can make a folder: a file added one by one into a
 * folder made here is the first Entry under it, and until that commit the
 * Library has no such folder to name. So the tree is asked again too, for the
 * sync as for the freeze — without it a folder a one-by-one add made stayed
 * out of the tree until somebody pressed "look for what is new".
 *
 * Each key names the run beside its status, the way a fill's count is in its
 * key. A freeze is of one folder, so the second book of a session that was
 * over before the first poll of it would otherwise read `done` after `done`
 * and ask for nothing — leaving its pages saying "not in Library" and its
 * folder dimmed until something else happened to reload them; and a second
 * sync over before any poll saw it run is the same `done` after `done`.
 */
export function useRunReloads(
  sync: Sync | null,
  freeze: Freeze | null,
  reloadListing: () => void,
  reloadFolders: () => void,
): void {
  const syncing = sync === null ? null : `${sync.run}:${sync.status}`;
  useEffect(() => {
    if (syncing !== null) {
      reloadListing();
      reloadFolders();
    }
  }, [syncing, reloadListing, reloadFolders]);

  const packing = freeze === null ? null : `${freeze.folder}:${freeze.run}:${freeze.status}`;
  useEffect(() => {
    if (packing !== null) {
      reloadListing();
      reloadFolders();
    }
  }, [packing, reloadListing, reloadFolders]);
}
