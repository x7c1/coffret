// Stopping an add of a folder from the Activity panel before it commits
// (spec: LA-34, LA-35, LA-36).
//
// A book's folder is dropped onto the album and added as a Pack, with Storage
// paused, so the freeze carrying it in is under way and cannot reach its
// commit: every request it makes of Storage waits. The packing line in the
// Activity panel offers a Stop, and pressing it is a request the run answers
// by ending. What the journey asserts is what a person can check afterwards:
// the panel says nothing was added to the Library, the folder on this
// device's disk holds nothing the drop wrote — not even the folder it made —
// and the Library lists nothing new, before and after a reload.
//
// Last of the journeys, because it pauses the Storage every journey before it
// reads from.

import { access } from 'node:fs/promises';
import path from 'node:path';

import { addAsAPack, bookCalled, dropFolderOnto } from './books';
import { expect, inTree, openActivity, photo, row, setting, shot, test } from './journey';
import { pauseStorage, unpauseStorage } from './storage';

/** What the book this journey stops is called. */
const STOPPED = 'stopped-before-it-was-packed';

/**
 * What the panel keeps once the add has been undone on this device, named by
 * the folder the book was dropped onto.
 */
function nothingAdded(): string {
  return `stopped adding to ${setting.album} — nothing was added to the Library`;
}

/** How long the drop has to land and its freeze to be under way. */
const UNDER_WAY_MS = 60_000;

/** Where the album stands on this device's disk: below the mapped prefix's folder. */
function albumOnDisk(): string {
  const [, ...below] = setting.album.split('/');
  return path.join(setting.deviceRoot, ...below);
}

/** Whether anything stands at `local`. */
async function stands(local: string): Promise<boolean> {
  try {
    await access(local);
    return true;
  } catch {
    return false;
  }
}

test('stop an add of a folder, and find nothing added and nothing left behind', async ({
  page,
}) => {
  const book = await bookCalled(STOPPED);
  const landed = path.join(albumOnDisk(), STOPPED);
  expect(await stands(landed)).toBe(false);

  await pauseStorage(setting);
  try {
    await page.goto(`/#path=${setting.album}`);
    await dropFolderOnto(row(page, photo(0)), book);
    await addAsAPack(page, STOPPED);

    // The pages landed and the freeze carrying them in is under way, held
    // before its commit: its line offers a Stop.
    await openActivity(page);
    const stop = page.getByRole('button', { name: /^Stop — packing / });
    await expect(stop).toBeVisible({ timeout: UNDER_WAY_MS });
    await shot(page, '01-the-add-under-way-offers-a-stop');
    await stop.click();

    // Answered by the run ending: the line goes, and the panel keeps what the
    // stop came to, with nothing to press again.
    const answered = page.getByText(nothingAdded());
    await expect(answered).toBeVisible({ timeout: UNDER_WAY_MS });
    await expect(page.getByRole('button', { name: /^Stop — / })).toHaveCount(0);
    await shot(page, '02-stopped-nothing-was-added');
  } finally {
    // Whatever became of the steps above, Storage answers again.
    await unpauseStorage(setting);
  }

  // Nothing the drop wrote is left on this device: no page, and not the
  // folder it made for them.
  expect(await stands(landed)).toBe(false);

  // And the Library lists nothing new, before a reload and after one, while
  // the server's log still says what the stop came to.
  await expect(inTree(page, STOPPED)).toHaveCount(0);
  await page.reload();
  await expect(row(page, photo(0))).toBeVisible();
  await expect(inTree(page, STOPPED)).toHaveCount(0);
  await openActivity(page);
  await expect(page.getByText(nothingAdded())).toBeVisible();
  await shot(page, '03-after-a-reload-the-library-has-nothing-new');
});
