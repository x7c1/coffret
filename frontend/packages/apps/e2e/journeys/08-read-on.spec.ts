// Reading off the end of one folder into the next.
//
// A series is read folder by folder, and the reader does not stop at the end
// of one: `↓` on its last file goes on to the first file of the next folder,
// opening it in the tree, as `↑` on the first goes back. The next folder is
// closed when the reading gets there, so which file comes first in it is the
// server's to say; and near the end of the first folder the reader has read
// the next one's first pages ahead, so the page is there when the key is
// pressed.
//
// The album is read into the book, which is the folder after it. Its last file
// is the one the restart journey had another device commit, named to sort after
// the photographs; reading it fetches it, as opening any file does.

import { ARRIVED_WHILE_DOWN, bookPage, expect, inTree, leaf, row, setting, shot, test, tree } from './journey';

test('read off the end of one folder into the first page of the next, and back', async ({ page }) => {
  const last = ARRIVED_WHILE_DOWN;
  await page.goto(`/#path=${setting.album}&open=${setting.album}/${last}`);
  const reader = page.getByRole('region', { name: 'Reader' });
  await expect(reader.getByRole('img', { name: last })).toBeVisible();
  // The keys are the tree's, and the page was opened from its address rather
  // than from a row.
  await tree(page).focus();
  // The book's folder is closed: the tree holds no listing of it yet.
  await expect(inTree(page, leaf(setting.book)).getByRole('button', { name: /^expand / })).toBeVisible();
  await shot(page, '01-last-photo-of-the-album');

  await page.keyboard.press('ArrowDown');
  await expect(reader.getByRole('img', { name: bookPage(0) })).toBeVisible();
  await expect(reader.getByLabel('position')).toHaveText(`1 / ${setting.pages}`);
  await expect(page).toHaveURL(new RegExp(`open=${setting.book}/${bookPage(0)}$`));
  // The folder it read on into is open, and the focused row in view.
  await expect(inTree(page, leaf(setting.book)).getByRole('button', { name: /^collapse / })).toBeVisible();
  await expect(row(page, bookPage(0))).toHaveAttribute('aria-selected', 'true');
  await expect(row(page, bookPage(0))).toBeInViewport();
  await shot(page, '02-first-page-of-the-book');

  await page.keyboard.press('ArrowDown');
  await expect(reader.getByRole('img', { name: bookPage(1) })).toBeVisible();

  // And back over the boundary, the folders read through left open.
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await expect(reader.getByRole('img', { name: last })).toBeVisible();
  await expect(row(page, last)).toHaveAttribute('aria-selected', 'true');
  await expect(inTree(page, leaf(setting.book)).getByRole('button', { name: /^collapse / })).toBeVisible();
  await shot(page, '03-back-in-the-album');
});
