// Opening one photograph, and finding the album came with it.
//
// Nobody who opens the first picture of an album stops there, so the server
// takes the rest of the folder up behind the reader — unasked, because there is
// nothing to ask with. What this journey watches is the rows changing under a
// reader that is still open on the first one.
//
// It has to run before anything else opens the album. The whole of what it says
// is that files nobody asked for arrived, and an album already on this disk
// could say that of a folder the server never touched — which is why the spec
// files are numbered and the worker is one.

import {
  expect,
  expectRemote,
  glimpse,
  ON_THIS_DEVICE,
  photo,
  row,
  setting,
  shot,
  stateIcon,
  test,
} from './journey';

/** How long the progress line is waited for before its picture is taken. */
const GLIMPSE_MS = 4_000;

/** How long the album has to come over a loopback MinIO. */
const BACKFILL_MS = 120_000;

test('open one photograph and watch the rest of the album arrive', async ({ page }) => {
  await page.goto(`/#path=${setting.album}`);

  // The album as this device has never seen it: every row is in the Library and
  // none of the files is here.
  const first = photo(0);
  const last = photo(setting.photos - 1);
  await expectRemote(page, first);
  await expectRemote(page, last);
  await shot(page, '01-nothing-here-yet');

  await row(page, first).dblclick();
  await expect(page.getByRole('img', { name: first })).toBeVisible();

  // The line the Activity panel shows while the folder is coming over. Photographed
  // where it is caught and not asserted: see `glimpse`.
  await glimpse(page.getByText(/bringing over/), GLIMPSE_MS);
  await shot(page, '02-bringing-the-album-over');

  // The reader is left open on purpose. It is what the page follows the work
  // through — closing it while the fill is still running would be asking a
  // different question of the screen than this journey is asking — and the last
  // photograph of the album is far enough from the first that no prefetch of
  // the reader's own reaches it. Its arriving is the fill and nothing else.
  await expect(stateIcon(page, last)).toHaveAccessibleName(ON_THIS_DEVICE, { timeout: BACKFILL_MS });
  await expect(stateIcon(page, photo(1))).toHaveAccessibleName(ON_THIS_DEVICE);

  await page.keyboard.press('Escape');
  await expect(page.getByText(/bringing over/)).toHaveCount(0);
  await shot(page, '03-the-album-is-here');
});
