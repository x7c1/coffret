// Somebody walks away from the machine, and shuts the Library behind them.
//
// The Passphrase was spent once, when the server was started, and until this
// journey every other one has been reading a Library those keys keep open. That
// is the state the lock ends (spec: DK-1, DK-3): one control beside the
// Library's name, and from the moment it answers nothing that needs the Master
// Key is served — the rows, the tree, the reader, the drop, all of them refused
// with the same sentence, which says the Passphrase is required and where to
// give it.
//
// It is the last journey because it is a one-way door. There is no unlock in the
// browser and there is deliberately not going to be one: the Passphrase is typed
// at a terminal, so what opens this server again is starting it again — which is
// the suite's teardown here rather than a step of its own.
//
// What is asserted is what a person would see. That the routes are shut is the
// router cases' business and the API stage's; this is the screen saying so.

import { expect, photo, row, setting, shot, test } from './journey';

/** The words the server's refusal is written around (spec: DK-2). */
const SAID = 'the Passphrase is required';

test('lock the Library and be told what it takes to open it', async ({ page }) => {
  await page.goto(`/#path=${setting.album}`);
  // A folder of rows, read out of a Library that is open.
  await expect(row(page, photo(1))).toHaveCount(1);
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  await shot(page, '01-open');

  await page.getByRole('button', { name: 'lock', exact: true }).click();

  // The screen asks its three questions again, and two of them are now refused.
  // The sentence is the server's own and is shown where every refusal on this
  // screen is shown — in the region that cannot answer, with the offer to ask
  // again beside it.
  await expect(page.getByText(SAID, { exact: false }).first()).toBeVisible();
  await expect(row(page, photo(1))).toHaveCount(0);

  // And the Library still has a name. Which Library this is was never something
  // the Master Key kept, so a locked server goes on saying it rather than going
  // silent — otherwise a person with two of them open would have no way to tell
  // which one they had just shut.
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  await shot(page, '02-locked');
});
