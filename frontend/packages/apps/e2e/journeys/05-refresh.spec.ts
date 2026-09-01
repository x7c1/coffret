// Another device adds a photograph, and this one is told when it asks.
//
// Everything on the screen comes out of this device's catalog, and a catalog
// only moves when the Journal is replayed into it (spec: CK-9). Nothing does
// that on a timer — the explorer makes no request while nothing is happening,
// deliberately — so a Container another device committed is invisible here until
// somebody asks for it. This journey is that asking: the other device commits,
// the folder on the screen says nothing about it, and the control in the status
// bar brings it in.
//
// What arrives is the row and not the file. A catch-up opens no Container, so
// the photograph is `remote` afterwards exactly as every other Entry this device
// has not fetched is (spec: EP-10) — and opening it is what brings the bytes,
// the way it always is.

import { chip, expect, leaf, row, setting, shot, test } from './journey';
import { commitElsewhere } from './uploader';

/** What the other device commits, named so that it sorts to the top of the album. */
const ADDED = 'committed-elsewhere.jpg';

test('ask what is new and find what another device committed', async ({ page }) => {
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, ADDED)).toHaveCount(0);
  await shot(page, '01-before-the-other-device-commits');

  await commitElsewhere(setting, `${setting.album}/${ADDED}`, setting.refreshFile);

  // Still nothing, and that is the point rather than a delay: this device has
  // not been told, and no amount of waiting would tell it.
  await expect(row(page, ADDED)).toHaveCount(0);

  await page.getByRole('button', { name: 'look for what is new' }).click();

  await expect(row(page, ADDED)).toHaveCount(1);
  await expect(chip(page, ADDED)).toHaveText('remote');
  await expect(page.getByText('1 new file')).toBeVisible();
  await shot(page, '02-the-catalog-caught-up');

  // The tree is asked again as well, so a folder that arrived with the same
  // commit would be in it. Nothing about the folder on the screen has otherwise
  // moved: the rows this device already had are as they were.
  await expect(page.getByTitle(leaf(setting.album), { exact: true }).first()).toBeVisible();

  // And the bytes come the way they always do.
  await row(page, ADDED).click();
  await expect(page.getByRole('img', { name: ADDED })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(chip(page, ADDED)).toHaveText('present');
  await shot(page, '03-and-then-it-is-here');
});
