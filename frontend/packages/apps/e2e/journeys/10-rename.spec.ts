// Renaming a folder from its row's menu.
//
// "Rename…" asks for the new name in a dialog holding the current one,
// selected, and refuses one the folder it stands in already holds with the
// reason under the field; a name it takes is one rename in the Library, made
// through the move route the drag uses. The folder renamed is the book the
// drag journey moved into the album.

import { expect, inTree, leaf, openActivity, photo, setting, shot, test, tree } from './journey';

test('rename a folder from its row’s menu', async ({ page }) => {
  const book = leaf(setting.book);
  const moved = `${setting.album}/${book}`;
  const renamed = `${book}-renamed`;
  await page.goto(`/#path=${moved}`);
  await expect(inTree(page, book)).toBeVisible();

  await inTree(page, book).click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Rename…' }).click();
  const dialog = page.getByRole('dialog', { name: `Rename “${book}”` });
  const field = dialog.getByLabel('new name');
  await expect(field).toHaveValue(book);

  // A name the album holds already is refused in the dialog, and nothing moves.
  await field.fill(photo(0));
  await dialog.getByRole('button', { name: 'Rename', exact: true }).click();
  await expect(dialog.getByRole('alert')).toHaveText(`there is already something called ${photo(0)} here`);
  await shot(page, '01-refused');

  await field.fill(renamed);
  await dialog.getByRole('button', { name: 'Rename', exact: true }).click();
  await expect(dialog).toHaveCount(0);

  await expect(tree(page).locator(`tr[data-key="folder:${setting.album}/${renamed}"]`)).toBeVisible();
  await expect(tree(page).locator(`tr[data-key="folder:${moved}"]`)).toHaveCount(0);
  await openActivity(page);
  await expect(page.locator('footer')).toContainText(`moved ${moved} → ${setting.album}/${renamed}`);
  await shot(page, '02-renamed');
});
