// Dragging a photograph onto a folder, which is the whole of adding one.
//
// There is no upload button: what a person means by dragging a file onto a
// folder is not in doubt, and the one thing the screen asks before anything is
// sent is the confirmation every drop gets — where it lands, and that a file on
// its own is added as it is. What the screen owes them afterwards is what
// follows — the file is in the folder while the Library does not have it yet,
// the sync the server armed carries it in, and then the Library has it — and
// this journey is that. How much of the middle is ever on the screen is
// the machine's speed, not the product's behavior: the sync is armed the
// moment the file lands, and on a slow enough runner it has committed before
// the browser draws the row at all, so the first state it ever shows is
// on this device. The journey therefore asserts the row and where it ends up, and
// the staged state — the ring that says it is being added, the backing-up line — is
// photographed where it is caught and never waited into existence.

import path from 'node:path';

import {
  BEING_ADDED,
  dropFileOnto,
  expect,
  glimpse,
  ON_THIS_DEVICE,
  photo,
  row,
  setting,
  shot,
  stateIcon,
  test,
} from './journey';

/** How long the staged state is waited for before the first picture. */
const GLIMPSE_MS = 4_000;

/** How long a photograph has to reach MinIO and be committed. */
const SYNC_MS = 120_000;

test('drop a photograph on the album and watch it become an Entry', async ({ page }) => {
  const dropped = path.basename(setting.dropFile);
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, dropped)).toHaveCount(0);

  // Onto the rows, which is the whole of the gesture: the folder on the screen
  // is the folder the files go to. The first row rather than the middle of the
  // list, only so that the point dropped on is on the screen however long the
  // folder is.
  await dropFileOnto(page, row(page, photo(0)), setting.dropFile);

  // Confirmed before anything is sent: one file, added on its own, into the
  // album the list is showing. Nothing is in the folder until Add.
  const question = page.getByRole('dialog');
  await expect(question).toContainText('1 file');
  await expect(question).toContainText('added on its own');
  await expect(question).toContainText(`goes into ${setting.album}/`);
  await expect(question).not.toContainText('is new');
  await expect(row(page, dropped)).toHaveCount(0);
  await question.getByRole('button', { name: 'Add', exact: true }).click();

  // In the folder from that moment. Which state its icon says is the race
  // described above — being added until the armed sync commits, on this device
  // where the sync outran the first frame — so the row and the sanity of its
  // state are asserted, and the moment it is being added is photographed where
  // it is caught: see `glimpse`.
  await expect(row(page, dropped)).toHaveCount(1);
  await expect(stateIcon(page, dropped)).toHaveAccessibleName(/^On this device/);
  await glimpse(row(page, dropped).getByRole('img', { name: BEING_ADDED }), GLIMPSE_MS);
  await shot(page, '01-landed');

  // And once it is committed the row is an ordinary one: the Library holds the
  // Entry, and this device has the file it was made from.
  await expect(stateIcon(page, dropped)).toHaveAccessibleName(ON_THIS_DEVICE, { timeout: SYNC_MS });
  await expect(page.getByText(/backing up what was added/)).toHaveCount(0);
  await shot(page, '02-in-the-library');
});
