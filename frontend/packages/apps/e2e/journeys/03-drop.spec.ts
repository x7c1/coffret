// Dragging a photograph onto a folder, which is the whole of adding one.
//
// There is no upload button and no dialog: what a person means by dragging a
// file onto a folder is not in doubt. What the screen owes them afterwards is
// the three states that follow — the file is in the folder, the Library does
// not have it yet, and now it does — and this journey is those three.

import path from 'node:path';

import { chip, dropFileOnto, expect, glimpse, photo, row, setting, shot, test } from './journey';

/** How long the line about the sync is waited for before its picture. */
const GLIMPSE_MS = 4_000;

/** How long a photograph has to reach MinIO and be committed. */
const SYNC_MS = 120_000;

test('drop a photograph on the album and watch it become an Entry', async ({ page }) => {
  const dropped = path.basename(setting.dropFile);
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, dropped)).toHaveCount(0);

  // Onto the rows, which is the whole of the gesture: the folder on the screen
  // is the folder the files go to. The first row rather than the middle of the
  // list, only so that the point dropped on is on the screen however long the
  // folder is.
  await dropFileOnto(page, row(page, photo(0)), setting.dropFile);

  // In the folder from that moment. Nothing has been committed for it, so the
  // Library holds no Entry there and the row says exactly that.
  await expect(chip(page, dropped)).toHaveText('uploading');
  await shot(page, '01-in-the-folder');

  // The sync the server armed as it took the file. Photographed where it is
  // caught and not asserted: see `glimpse`.
  await glimpse(page.getByText(/backing up what was added/), GLIMPSE_MS);
  await shot(page, '02-backing-it-up');

  // And once it is committed the row is an ordinary one: the Library holds the
  // Entry, and this device has the file it was made from.
  await expect(chip(page, dropped)).toHaveText('present', { timeout: SYNC_MS });
  await expect(page.getByText(/backing up what was added/)).toHaveCount(0);
  await shot(page, '03-in-the-library');
});
