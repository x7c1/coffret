// Another device adds a photograph, and this one is told when it asks.
//
// Everything on the screen comes out of this device's catalog, and a catalog
// only moves when the Journal is replayed into it (spec: CK-9). Nothing does
// that on a timer — the explorer only looks, by name, whether there is a newer
// head, and draws the sync icon as waiting when there is — so a Container
// another device committed is not on the screen until somebody asks for it.
// This journey is that asking: the other device commits, the folder on the
// screen says nothing about it, the icon says changes are waiting, and pressing
// it brings them in.
//
// What arrives is the row and not the file. A catch-up opens no Container, so
// the photograph is `remote` afterwards exactly as every other Entry this device
// has not fetched is (spec: EP-10) — and opening it is what brings the bytes,
// the way it always is.

import {
  expect,
  expectRemote,
  inTree,
  leaf,
  ON_THIS_DEVICE,
  row,
  setting,
  shot,
  stateIcon,
  test,
} from './journey';
import { commitElsewhere } from './uploader';

/**
 * What the other device commits, named so that it sorts after the album's
 * photographs. The album is the focused folder, and the details pane fetches a
 * focused folder's cover — its first image — where this device does not have
 * it (spec: LA-33). Named to sort first, this file would be that cover, and on
 * this device before anybody opened it.
 */
const ADDED = 'then-committed-elsewhere.jpg';

/** What the sync icon is called, plain and while changes are waiting. */
const CHECKING = 'Check for changes';
const CHANGES_WAITING = `${CHECKING} — changes are waiting`;

test('ask what is new and find what another device committed', async ({ page }) => {
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, ADDED)).toHaveCount(0);
  await shot(page, '01-before-the-other-device-commits');

  await commitElsewhere(setting, `${setting.album}/${ADDED}`, setting.refreshFile);

  // Still nothing, and that is the point rather than a delay: this device has
  // not caught up, and no amount of waiting would bring the row in. What the
  // explorer learns on its own — here as the page comes up again — is that
  // there is something to bring in.
  await expect(row(page, ADDED)).toHaveCount(0);
  await page.reload();
  await expect(page.getByRole('button', { name: CHANGES_WAITING })).toBeVisible();
  await expect(row(page, ADDED)).toHaveCount(0);

  await page.getByRole('button', { name: CHANGES_WAITING }).click();

  await expect(row(page, ADDED)).toHaveCount(1);
  await expectRemote(page, ADDED);
  // Below the album's photographs, so brought on to the screen for the picture.
  await row(page, ADDED).scrollIntoViewIfNeeded();
  await expect(page.getByText('1 new file', { exact: true })).toBeVisible();
  // And the Activity row says it as the newest thing that ended, since a
  // catch-up the explorer asked for is in the server's log like other work.
  await expect(page.getByText('checked for changes — 1 new file')).toBeVisible();
  await expect(page.getByRole('button', { name: CHECKING, exact: true })).toBeVisible();
  await shot(page, '02-the-catalog-caught-up');

  // The tree is asked again as well, so a folder that arrived with the same
  // commit would be in it. Nothing about the folder on the screen has otherwise
  // moved: the rows this device already had are as they were.
  await expect(inTree(page, leaf(setting.album))).toBeVisible();

  // And the bytes come the way they always do.
  await row(page, ADDED).dblclick();
  await expect(page.getByRole('img', { name: ADDED })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(stateIcon(page, ADDED)).toHaveAccessibleName(ON_THIS_DEVICE);
  await shot(page, '03-and-then-it-is-here');
});
