// Opening the explorer, walking down to a folder, and reading it.
//
// The first journey, and the one every other journey stands on: it is what says
// the joined device's Library is on the screen at all — the tree the server
// answered with, a folder's rows, and a page of a book large enough to read.
//
// It is also the one that says the URL is where the screen's place lives.
// Reloading in the middle of a book is the ordinary thing a person does to a
// tab, and coming back to the list would lose where they were.

import {
  bookPage,
  expect,
  expectRemote,
  leaf,
  libraryName,
  openInTree,
  row,
  rowsNamed,
  setting,
  shot,
  test,
  top,
} from './journey';

test('walk the tree to a book, read a page, and reload back into it', async ({ page }) => {
  await page.goto('/');

  // The status bar names the Library this device joined, under the name this
  // device gave it.
  await expect(libraryName(page)).toBeVisible();
  const mapped = top(setting.album);
  await expect(row(page, mapped)).toBeVisible();
  await shot(page, '01-library-root');

  // Down the tree, one folder at a time: a folder two components deep is only
  // on the screen with the one above it open.
  await openInTree(page, mapped);
  await openInTree(page, leaf(setting.book));
  await expect(page).toHaveURL(new RegExp(`#path=${setting.book}$`));

  // Every page of the book is in the Library and none of them is on this device
  // yet: this device joined and caught its Index up, and has fetched nothing.
  // Every page is a row of its own, under the book's.
  await expectRemote(page, bookPage(0));
  const pages = Array.from({ length: setting.pages }, (_, index) => bookPage(index));
  await expect(rowsNamed(page, pages)).toHaveCount(setting.pages);
  await shot(page, '02-book-folder');

  // A click selects a row and opens nothing.
  await row(page, bookPage(0)).click();
  await expect(row(page, bookPage(0))).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('img', { name: bookPage(0) })).toHaveCount(0);

  // Opening a page fetches it, which is the reader's whole vocabulary for a
  // file this device does not have: there is no download button to press.
  await row(page, bookPage(0)).dblclick();
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
  await row(page, bookPage(2)).dblclick();
  await expect(page.getByRole('img', { name: bookPage(2) })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('img', { name: bookPage(2) })).toBeVisible();
  await expect(page.getByText(`${bookPage(2)} (3/${setting.pages})`)).toBeVisible();
  await shot(page, '06-reloaded-into-the-reader');
});
