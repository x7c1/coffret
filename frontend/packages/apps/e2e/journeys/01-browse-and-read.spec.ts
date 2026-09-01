// Opening the explorer, walking down to a folder, and reading it.
//
// The first journey, and the one every other journey stands on: it is what says
// the joined device's Library is on the screen at all — the tree the server
// answered with, a folder's rows, and a page of a book large enough to read.
//
// It is also the one that says the URL is where the screen's place lives.
// Reloading in the middle of a book is the ordinary thing a person does to a
// tab, and coming back to the list would lose where they were.

import { bookPage, chip, expect, inTree, leaf, row, setting, shot, test, top } from './journey';

test('walk the tree to a book, read a page, and reload back into it', async ({ page }) => {
  await page.goto('/');

  // The status bar names the Library this device joined, under the name this
  // device gave it and on the provider it is kept on.
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  const mapped = top(setting.album);
  await expect(row(page, mapped)).toBeVisible();
  await shot(page, '01-library-root');

  // Down the tree rather than through the rows: the tree is the one part of the
  // screen that draws the whole Library, and a folder two components deep is
  // only reachable with the one above it open.
  await inTree(page, mapped).click();
  await inTree(page, leaf(setting.book)).click();
  await expect(page).toHaveURL(new RegExp(`#path=${setting.book}$`));

  // Every page of the book is in the Library and none of them is on this device
  // yet: this device joined and caught its Index up, and has fetched nothing.
  await expect(chip(page, bookPage(0))).toHaveText('remote');
  await expect(page.locator('tbody tr')).toHaveCount(setting.pages);
  await shot(page, '02-book-folder');

  // Opening a page fetches it, which is the reader's whole vocabulary for a
  // file this device does not have: there is no download button to press.
  await row(page, bookPage(0)).click();
  await expect(page.getByRole('img', { name: bookPage(0) })).toBeVisible();
  await expect(page.getByText(`${bookPage(0)} (1/${setting.pages})`)).toBeVisible();
  await shot(page, '03-first-page');

  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('img', { name: bookPage(1) })).toBeVisible();
  await expect(page.getByText(`${bookPage(1)} (2/${setting.pages})`)).toBeVisible();
  await shot(page, '04-second-page');

  await page.keyboard.press('ArrowLeft');
  await expect(page.getByRole('img', { name: bookPage(0) })).toBeVisible();

  // Escape comes back to the list, with the row that was open still marked.
  await page.keyboard.press('Escape');
  await expect(page.getByRole('img', { name: bookPage(0) })).toHaveCount(0);
  await expect(page).toHaveURL(new RegExp(`#path=${setting.book}$`));
  await shot(page, '05-back-to-the-list');

  // And the URL is where the screen's place lives: a reload in the middle of a
  // book comes back to the page that was open and not to the top of the folder.
  await row(page, bookPage(2)).click();
  await expect(page.getByRole('img', { name: bookPage(2) })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('img', { name: bookPage(2) })).toBeVisible();
  await expect(page.getByText(`${bookPage(2)} (3/${setting.pages})`)).toBeVisible();
  await shot(page, '06-reloaded-into-the-reader');
});
