// Reading on inside a filtered tree.
//
// While the tree is filtered by a name, `↓` past the last page of one book the
// name matched goes on to the first page of the next book it matched — not
// into a book between them in the Library that it did not match, which the
// filtered tree does not show. Past the last page of the last match there is
// nothing inside further, and the reader stays where it is.
//
// Three books of their own are dropped into the album for it, so that which
// two matched and which one stands between them is this journey's to say: a
// name, `kept`, matches the first and the third, and the second is passed
// over.

import { readdir } from 'node:fs/promises';

import type { Page } from '@playwright/test';

import { bookCalled, dropAsAPack } from './books';
import { expect, inTree, ON_THIS_DEVICE, row, setting, shot, test, tree } from './journey';

/** The three books, in the Library's order: the first and the last match the name. */
const FIRST = 'read-on-a-kept';
const BETWEEN = 'read-on-b-passed';
const LAST = 'read-on-c-kept';

/** How long a book has to reach MinIO and be committed. */
const FREEZE_MS = 120_000;

/** Drops a book called `name` onto the album, and waits for the Library to hold its pages. */
async function shelved(page: Page, name: string, pages: readonly string[]): Promise<void> {
  await dropAsAPack(page, await bookCalled(name));
  await expect(inTree(page, name)).toBeVisible({ timeout: FREEZE_MS });
  await page.goto(`/#path=${setting.album}/${name}`);
  // Each by its key: the books dropped before stay open, and their pages carry the same names.
  for (const one of pages) {
    const icon = page
      .locator(`tbody tr[data-key="file:${setting.album}/${name}/${one}"]`)
      .getByRole('img', { name: ON_THIS_DEVICE });
    await expect(icon).toBeVisible({ timeout: FREEZE_MS });
  }
}

/** Where the reader is, as the address names it. */
function reading(book: string, name: string): RegExp {
  return new RegExp(`open=${setting.album}/${book}/${name}$`);
}

test('read on from one book a name matched into the next, past one it did not', async ({ page }) => {
  // Three books committed, each in its own time.
  test.setTimeout(3 * FREEZE_MS + 60_000);
  const pages = (await readdir(setting.importDir)).sort();
  for (const name of [FIRST, BETWEEN, LAST]) {
    await shelved(page, name, pages);
  }

  const field = page.getByRole('search').getByRole('textbox', { name: 'Filter by name' });
  await field.fill('kept');
  await expect(page.getByRole('status', { name: 'filter' })).toContainText(/^2 matches/);
  await expect(inTree(page, FIRST)).toBeVisible();
  await expect(inTree(page, LAST)).toBeVisible();
  await expect(inTree(page, BETWEEN)).toHaveCount(0);

  // The first match opened, and its first page in the reader.
  await inTree(page, FIRST).getByRole('button', { name: `expand ${FIRST}` }).click();
  await row(page, pages[0]).dblclick();
  const reader = page.getByRole('region', { name: 'Reader' });
  await expect(reader.getByRole('img', { name: pages[0] })).toBeVisible();
  await expect(page).toHaveURL(reading(FIRST, pages[0]));
  await tree(page).focus();
  await shot(page, '01-the-first-match-in-the-reader');

  // Down its pages, and past its last: onto the first page of the next match,
  // its folder opened, the book between passed over.
  for (const name of pages.slice(1)) {
    await page.keyboard.press('ArrowDown');
    await expect(page).toHaveURL(reading(FIRST, name));
  }
  await page.keyboard.press('ArrowDown');
  await expect(page).toHaveURL(reading(LAST, pages[0]));
  await expect(reader.getByRole('img', { name: pages[0] })).toBeVisible();
  await expect(inTree(page, LAST).getByRole('button', { name: `collapse ${LAST}` })).toBeVisible();
  await expect(inTree(page, BETWEEN)).toHaveCount(0);
  await shot(page, '02-on-into-the-next-match');

  // Down to the last page of the last match, and past it: nothing inside the
  // filtered tree comes after, so the reader stays.
  for (const name of pages.slice(1)) {
    await page.keyboard.press('ArrowDown');
    await expect(page).toHaveURL(reading(LAST, name));
  }
  const end = pages[pages.length - 1];
  await page.keyboard.press('ArrowDown');
  // Nothing is drawn below it either, so there is nothing to ask the server.
  await expect(page).toHaveURL(reading(LAST, end));
  await expect(reader.getByLabel('position')).toHaveText(`${pages.length} / ${pages.length}`);
  await expect(reader.getByRole('img', { name: end })).toBeVisible();
  await shot(page, '03-stayed-at-the-last-match');
});
