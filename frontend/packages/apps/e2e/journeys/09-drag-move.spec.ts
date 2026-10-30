// Moving a folder by dragging its row onto another folder.
//
// A person rearranging a Library drags: the book's row onto the folder it
// belongs in. Nothing is written before the confirmation the drop brings up —
// built from the server's preview — has been answered, and what Move writes
// is one rename in the Library: nothing is uploaded, and this device's files
// follow, since both folders stand under one mapped folder. Once the server
// answers, the tree shows the folder at its new place, and the Activity panel
// says the move.
//
// The book is moved into the album, both top-level folders' children: the
// journeys before this one are done reading either of them.

import { expect, inTree, leaf, openActivity, setting, shot, test, top, tree } from './journey';

test('drag the book’s folder onto the album, confirm, and find it inside the album', async ({ page }) => {
  const book = leaf(setting.book);
  const album = leaf(setting.album);
  await page.goto(`/#path=${top(setting.book)}`);
  await expect(inTree(page, book)).toBeVisible();
  await expect(inTree(page, album)).toBeVisible();

  await inTree(page, book).dragTo(inTree(page, album));

  // Asked first, with Cancel focused: what the move does, from the preview.
  const question = page.getByRole('dialog', { name: `Move “${book}” to “${album}”?` });
  await expect(question).toContainText(`${setting.pages} files will be given a new path in the Library`);
  await expect(question.getByRole('button', { name: 'Cancel' })).toBeFocused();
  await shot(page, '01-asked');
  await question.getByRole('button', { name: 'Move', exact: true }).click();
  await expect(question).toHaveCount(0);

  // At its new place, the album opened down to it and the focus on it.
  const moved = `${setting.album}/${book}`;
  await expect(tree(page).locator(`tr[data-key="folder:${moved}"]`)).toBeVisible();
  await expect(tree(page).locator(`tr[data-key="folder:${setting.book}"]`)).toHaveCount(0);
  await expect(inTree(page, book)).toHaveAttribute('aria-selected', 'true');
  await openActivity(page);
  await expect(page.locator('footer')).toContainText(`moved ${setting.book} → ${moved}`);
  await shot(page, '02-moved');
});
