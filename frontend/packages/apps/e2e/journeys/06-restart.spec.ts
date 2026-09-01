// A server that starts finds the Library as it is now, not as it was.
//
// The other half of the catch-up, and the one nobody presses: a device that has
// just joined holds an empty catalog, and one whose process has been stopped for
// a while holds the Library as it stood when it last looked. Either would open
// an explorer showing something that is not the Library — and would go on
// showing it for as long as the process ran, since nothing follows the remote
// head on its own.
//
// So the server asks once as it starts. This journey is that, with the interval
// made visible: the other device commits while this one's server is down, and
// what comes back has the row without anybody asking for it.
//
// It is also what the whole run rests on quietly. This device joined the Library
// from a Recovery Code and no `coffret sync` was ever run on it before the server
// started, so every folder the earlier journeys walked was on the screen because
// of a startup catch-up.

import { chip, expect, row, setting, shot, test } from './journey';
import { commitElsewhere } from './uploader';

/** What the other device commits while nothing here is listening. */
const ADDED = 'arrived-while-the-server-was-down.jpg';

test('come back to a Library another device has moved on', async ({ page, coffret }) => {
  await page.goto(`/#path=${setting.album}`);
  await expect(row(page, ADDED)).toHaveCount(0);

  await coffret.stop();
  await commitElsewhere(setting, `${setting.album}/${ADDED}`, setting.restartFile);
  await shot(page, '01-committed-while-nothing-was-listening');

  await coffret.start();
  await page.reload();

  // No button pressed and nothing polled: the server caught the catalog up as it
  // opened the Library, so the first listing it answers is the Library as it is.
  await expect(row(page, ADDED)).toHaveCount(1);
  await expect(chip(page, ADDED)).toHaveText('remote');
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  await shot(page, '02-the-row-was-there-on-the-first-listing');
});
