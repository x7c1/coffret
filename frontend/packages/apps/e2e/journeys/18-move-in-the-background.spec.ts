// A move carried on as background work while Storage keeps it waiting
// (spec: LA-31).
//
// A photograph of the album is dragged onto the book folder beside it. The
// confirmation is the preview's, as ever; then Storage is paused, so that the
// move, once accepted, waits in its catch-up for as long as the journey holds
// it. The confirmation closes at once all the same, and the tree draws the
// photograph at its new place as being moved — and not at its old one — with
// the Activity panel saying the move is under way. Once Storage answers again
// the move goes through, and the row is an ordinary row of the Library there.
//
// After the journeys that read the album, the drag journey and the rename
// journey: the book folder is the one they moved and renamed.

import { expect, inTree, leaf, openActivity, photo, row, setting, shot, test, tree } from './journey';
import { pauseStorage, unpauseStorage } from './storage';

/** How long the move has to go through once Storage answers again. */
const SETTLE_MS = 60_000;

test('move a photograph while Storage keeps the move waiting, and watch it settle', async ({ page }) => {
  const book = `${leaf(setting.book)}-renamed`;
  const from = `${setting.album}/${photo(1)}`;
  const to = `${setting.album}/${book}/${photo(1)}`;
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, photo(1))).toBeVisible();
  await expect(inTree(page, book)).toBeVisible();

  await row(page, photo(1)).dragTo(inTree(page, book));
  const question = page.getByRole('dialog', { name: `Move “${photo(1)}” to “${book}”?` });
  await expect(question).toContainText('1 file will be given a new path in the Library');

  await pauseStorage(setting);
  try {
    await question.getByRole('button', { name: 'Move', exact: true }).click();
    // Closed at once: the server accepted the move, and carries it on.
    await expect(question).toHaveCount(0);

    // Drawn at its new place as being moved, and not at its old one.
    const moving = tree(page).locator(`tr[data-key="file:${to}"]`);
    await expect(moving).toHaveAttribute('aria-busy', 'true');
    await expect(moving).toContainText('being moved…');
    await expect(tree(page).locator(`tr[data-key="file:${from}"]`)).toHaveCount(0);
    await openActivity(page);
    await expect(page.locator('footer')).toContainText(`moving ${from} → ${to}`);
    await shot(page, '01-drawn-as-being-moved');
  } finally {
    // Whatever became of the steps above, Storage answers again.
    await unpauseStorage(setting);
  }

  // The move went through: an ordinary row of the Library at its new place.
  const settled = tree(page).locator(`tr[data-key="file:${to}"]`);
  await expect(settled).not.toHaveAttribute('aria-busy', 'true', { timeout: SETTLE_MS });
  await expect(settled).toBeVisible();
  await expect(settled).not.toContainText('being moved…');
  await expect(tree(page).locator(`tr[data-key="file:${from}"]`)).toHaveCount(0);
  await expect(page.locator('footer')).toContainText(`moved ${from} → ${to}`);
  await shot(page, '02-settled');
});
