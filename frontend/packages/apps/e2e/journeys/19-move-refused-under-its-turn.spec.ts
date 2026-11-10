// A move refused once its turn came, which puts the rows back (spec: LA-31).
//
// A photograph of the album is dragged onto the book folder, and its preview
// admits the move. Storage is paused before Move is pressed, so the move,
// accepted at once and drawn at its new place, waits in its catch-up; while it
// waits, the photograph is changed on this device's disk. Once Storage answers
// again the move asks its refusals again under its turn, finds the file edited
// — it would leave this device with what the Library does not hold — and
// stops before anything is written. The tree draws the photograph back where
// it was, and the Activity panel says why.
//
// Last of the journeys: it leaves an edited photograph in the album.

import { appendFile } from 'node:fs/promises';
import path from 'node:path';

import { expect, inTree, leaf, openActivity, photo, row, setting, shot, test, tree } from './journey';
import { pauseStorage, unpauseStorage } from './storage';

/** How long the move has to be refused once Storage answers again. */
const SETTLE_MS = 60_000;

/** Where a photograph of the album stands on this device's disk: below the mapped prefix's folder. */
function onDisk(name: string): string {
  const [, ...below] = setting.album.split('/');
  return path.join(setting.deviceRoot, ...below, name);
}

test('a move refused under its turn puts the photograph back, and the Activity panel says why', async ({ page }) => {
  const book = `${leaf(setting.book)}-renamed`;
  const from = `${setting.album}/${photo(2)}`;
  const to = `${setting.album}/${book}/${photo(2)}`;
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, photo(2))).toBeVisible();

  await row(page, photo(2)).dragTo(inTree(page, book));
  const question = page.getByRole('dialog', { name: `Move “${photo(2)}” to “${book}”?` });
  await expect(question).toContainText('1 file will be given a new path in the Library');

  await pauseStorage(setting);
  try {
    await question.getByRole('button', { name: 'Move', exact: true }).click();
    await expect(question).toHaveCount(0);
    await expect(tree(page).locator(`tr[data-key="file:${to}"]`)).toHaveAttribute('aria-busy', 'true');
    await expect(tree(page).locator(`tr[data-key="file:${from}"]`)).toHaveCount(0);
    // Changed on this device while the move waits for its turn.
    await appendFile(onDisk(photo(2)), 'edited while the move waited');
    await shot(page, '01-drawn-as-being-moved');
  } finally {
    await unpauseStorage(setting);
  }

  // Back where it was, and nowhere else.
  const back = tree(page).locator(`tr[data-key="file:${from}"]`);
  await expect(back).toBeVisible({ timeout: SETTLE_MS });
  await expect(back).not.toHaveAttribute('aria-busy', 'true');
  await expect(tree(page).locator(`tr[data-key="file:${to}"]`)).toHaveCount(0);
  await openActivity(page);
  await expect(page.locator('footer')).toContainText(`could not move ${from} → ${to}`);
  await expect(page.locator('footer')).toContainText('changed on this device since it was placed');
  await shot(page, '02-put-back-with-the-reason');
});
