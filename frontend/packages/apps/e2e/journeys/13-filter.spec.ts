// Filtering the tree from the header's search field.
//
// A name typed there is answered by the server out of the whole Library: the
// tree is drawn from that answer, each row it kept under the folders above it,
// drawn open and dimmed, a kept folder closed until it is opened — and then
// listed whole, as without a filter. A line over the rows says how many
// matched. "Recent" in the field's panel keeps the most recently added books
// instead, or among what the name matched, and the tree gains an Added
// column while it does. Clear turns both off, and the tree comes back as it
// stood before the filter: the folders open that were open.

import { expect, inTree, leaf, photo, row, setting, shot, test, top, tree } from './journey';

/** When something was added, as the Added column writes it. */
const A_TIME = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$/;

test('filter the tree by a name and by the most recent books, then clear it', async ({ page }) => {
  const album = leaf(setting.album);
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, photo(0))).toBeVisible();
  const field = page.getByRole('search').getByRole('textbox', { name: 'Filter by name' });
  const status = page.getByRole('status', { name: 'filter' });
  await expect(status).toHaveCount(0);

  // `/` puts the focus in the field from the tree.
  await tree(page).focus();
  await page.keyboard.press('/');
  await expect(field).toBeFocused();

  // A name: the album, kept and closed, under its top-level folder drawn open.
  await field.fill(album);
  await expect(status).toContainText(/^\d[\d,]* match(es)?/);
  await expect(inTree(page, album)).toBeVisible();
  await expect(inTree(page, album).getByRole('button', { name: `expand ${album}` })).toBeVisible();
  await expect(inTree(page, top(setting.album)).getByRole('button', { name: `collapse ${top(setting.album)}` })).toBeVisible();
  await expect(row(page, photo(0))).toHaveCount(0);
  await shot(page, '01-by-name');

  // Opened, a kept folder lists everything in it.
  await inTree(page, album).getByRole('button', { name: `expand ${album}` }).click();
  await expect(row(page, photo(0))).toBeVisible();
  await expect(row(page, photo(1))).toBeVisible();
  await shot(page, '02-kept-folder-opened');

  // The most recent book alone: the name emptied, Recent on with a count of 1.
  await field.fill('');
  await page.getByRole('button', { name: 'More filters' }).click();
  const panel = page.getByRole('group', { name: 'More filters' });
  await panel.getByRole('spinbutton', { name: 'How many recent folders' }).fill('1');
  await panel.getByRole('checkbox').check();
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(page.getByTestId('recent-chip')).toHaveText(/Recent 1/);
  await expect(status).toContainText(/^1 match/);
  await expect(tree(page).getByRole('columnheader', { name: 'Added' })).toBeVisible();
  // The book kept says when it was added; the folders above it say nothing.
  const added = tree(page).locator('tbody tr[data-key] td:nth-child(5)');
  await expect(added.filter({ hasText: A_TIME })).toHaveCount(1);
  await shot(page, '03-most-recent');

  // Clear: the name and Recent off, and the tree as it stood before.
  await status.getByRole('button', { name: 'Clear' }).click();
  await expect(status).toHaveCount(0);
  await expect(page.getByTestId('recent-chip')).toHaveCount(0);
  await expect(tree(page).getByRole('columnheader', { name: 'Added' })).toHaveCount(0);
  await expect(inTree(page, album).getByRole('button', { name: `collapse ${album}` })).toBeVisible();
  await expect(row(page, photo(0))).toBeVisible();
  await shot(page, '04-cleared');
});
