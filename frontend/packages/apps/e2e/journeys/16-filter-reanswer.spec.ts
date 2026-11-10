// A filtered tree asked again, its folders left as a person left them.
//
// While the tree is filtered, the filter is asked of the server again
// whenever the Library changes under it — here, a photograph another device
// committed, brought in by "Check for changes" while the tree is filtered by
// a name. (Not a drop: a drop opens the folder it lands in, so that the file
// being added shows, filter or not.) The answer to the same name is a new
// one, with a row the commit brought in; but the folders a person closed stay
// closed, the new row's among them: what arrived is the Activity panel's to
// say, not a tree sprung open again. Only another name, or another Recent,
// opens the folders above what it keeps.

import { clearFilter, expect, inTree, leaf, setting, shot, test, top } from './journey';
import { commitElsewhere } from './uploader';

/** How long the other device's commit and the catch-up have to take. */
const SYNC_MS = 120_000;

/** How many rows the line over the tree says matched. */
function matched(line: string): number {
  const count = /^(\d[\d,]*) match/.exec(line);
  if (count === null) {
    throw new Error(`the filter's line says no count: ${line}`);
  }
  return Number(count[1].replaceAll(',', ''));
}

test('the same filter answered again leaves the folders closed', async ({ page }) => {
  test.setTimeout(2 * SYNC_MS + 60_000);
  const album = leaf(setting.album);
  const above = top(setting.album);
  // A photograph whose name the filter matches, beside the album rather than
  // in it — a row under a matched folder is that folder's, not a match of its
  // own — so that the answer gains a row under the folder that is closed.
  const added = `${album}-committed-while-filtered.jpg`;
  const addedRow = page.locator(`tbody tr[title="${added}"]`);

  await page.goto('/');
  const field = page.getByRole('search').getByRole('textbox', { name: 'Filter by name' });
  const status = page.getByRole('status', { name: 'filter' });
  await field.fill(album);
  await expect(status).toContainText(/^\d[\d,]* match(es)?/);
  const before = matched((await status.textContent()) ?? '');
  const collapse = inTree(page, above).getByRole('button', { name: `collapse ${above}` });
  await expect(collapse).toBeVisible();
  await expect(inTree(page, album)).toBeVisible();

  // Every folder closed: the top-level folder above the album is all there is.
  await collapse.click();
  const expand = inTree(page, above).getByRole('button', { name: `expand ${above}` });
  await expect(expand).toBeVisible();
  await expect(inTree(page, album)).toHaveCount(0);
  await shot(page, '01-filtered-and-closed');

  // Another device commits the photograph beside the album, and this one checks
  // for changes while the tree is still filtered.
  await commitElsewhere(setting, `${above}/${added}`, setting.refreshFile);
  await page.getByRole('button', { name: /^Check for changes/ }).click();

  // The filter is answered again, with one more match — and the folder stays
  // closed, the album and the new row beside it undrawn.
  await expect(status).toContainText(new RegExp(`^${before + 1} match`), { timeout: SYNC_MS });
  await expect(expand).toBeVisible();
  await expect(inTree(page, album)).toHaveCount(0);
  await expect(addedRow).toHaveCount(0);
  await shot(page, '02-answered-again-still-closed');

  // Another name opens the folders above what it keeps again.
  await field.fill(added);
  await expect(status).toContainText(/^1 match/);
  await expect(collapse).toBeVisible();
  await expect(addedRow).toHaveCount(1);
  await shot(page, '03-another-name-opened');

  // Cleared, and kept cleared (spec: LA-43), so the journeys after this one
  // open unfiltered.
  await clearFilter(page);
});
