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
  inTree,
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

  // The Activity panel names the Library this device joined, under the name this
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
  await expect(page.getByRole('region', { name: 'Reader' })).toHaveCount(0);

  // Opening a page fetches it, which is the reader's whole vocabulary for a
  // file this device does not have: there is no download button to press. The
  // reader is a sheet over the right of the tree, and a strip of the tree stays
  // in sight beside it.
  await row(page, bookPage(0)).dblclick();
  const reader = page.getByRole('region', { name: 'Reader' });
  await expect(reader.getByRole('img', { name: bookPage(0) })).toBeVisible();
  await expect(reader.getByLabel('position')).toHaveText(`1 / ${setting.pages}`);
  await expect(row(page, bookPage(0))).toBeInViewport();
  await shot(page, '03-first-page');

  // The sheet follows the tree's focus, and the keys stay with the tree: the
  // next file is the next page.
  await page.keyboard.press('ArrowDown');
  await expect(reader.getByRole('img', { name: bookPage(1) })).toBeVisible();
  await expect(reader.getByLabel('position')).toHaveText(`2 / ${setting.pages}`);
  await expect(row(page, bookPage(1))).toHaveAttribute('aria-selected', 'true');
  await shot(page, '04-second-page');

  await page.keyboard.press('ArrowUp');
  await expect(reader.getByRole('img', { name: bookPage(0) })).toBeVisible();

  // With the sheet open the arrows go from file to file, so the book's folder
  // is reached by its row: a click moves the focus there, and the sheet shows
  // its cover, told apart from its first page, with the folder left open. Not
  // `↑` from the first page, which would read back into the last file of the
  // folder before it — the album, which the next journey needs untouched.
  // Clicked on its name: the rest of the row is under the sheet, and the strip
  // of the tree left uncovered is where the rows stay in reach.
  await inTree(page, leaf(setting.book)).getByText(leaf(setting.book), { exact: true }).click();
  await expect(reader.getByRole('img', { name: `cover of ${leaf(setting.book)}` })).toBeVisible();
  await expect(reader.getByRole('img', { name: bookPage(0) })).toHaveCount(0);
  await expect(reader.getByText(`Folder · ${setting.pages} items`, { exact: false })).toBeVisible();
  await expect(rowsNamed(page, pages)).toHaveCount(setting.pages);

  // Escape comes back to the tree, with the row the sheet was on still
  // marked.
  await page.keyboard.press('Escape');
  await expect(reader).toHaveCount(0);
  await expect(inTree(page, leaf(setting.book))).toHaveAttribute('aria-selected', 'true');
  await expect(page).toHaveURL(new RegExp(`#path=${setting.book}$`));
  await shot(page, '05-back-to-the-list');

  // And the URL is where the screen's place lives: a reload in the middle of a
  // book comes back to the page that was open and not to the top of the folder.
  await row(page, bookPage(2)).dblclick();
  await expect(reader.getByRole('img', { name: bookPage(2) })).toBeVisible();
  await page.reload();
  await expect(reader.getByRole('img', { name: bookPage(2) })).toBeVisible();
  await expect(reader.getByLabel('position')).toHaveText(`3 / ${setting.pages}`);
  await shot(page, '06-reloaded-into-the-reader');
});
