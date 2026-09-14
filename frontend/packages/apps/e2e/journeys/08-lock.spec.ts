// Somebody walks away from the machine, mid-book, and shuts the Library behind
// them.
//
// The Passphrase was spent once, when the server was started, and until this
// journey every other one has been reading a Library those keys keep open. That
// is the state the lock ends (spec: DK-1, DK-3): one control beside the
// Library's name, and from the moment it answers nothing that needs the Master
// Key is served — the rows, the tree, the reader, the drop, all of them refused
// with the same sentence, which says the Passphrase is required and where to
// give it.
//
// It is walked from inside the reader because that is where a person locking is
// most likely to be: a page of a book open and large, drawn from plaintext this
// tab is holding. What the journey is after is the page coming off the screen
// when the lock answers. The unit tests prove the order of the gesture, that the
// plaintext is given up before the screen asks anything; only a real browser can
// say the picture is gone.
//
// It is the last journey because it is a one-way door. There is no unlock in the
// browser and there is deliberately not going to be one: the Passphrase is typed
// at a terminal, so what opens this server again is starting it again — which is
// the suite's teardown here rather than a step of its own.
//
// What is asserted is what a person would see. That the routes are shut is the
// router cases' business and the API stage's; this is the screen saying so.

import {
  bookPage,
  expect,
  inTree,
  leaf,
  row,
  setting,
  shot,
  test,
  top,
} from './journey';

/** The words the server's refusal is written around (spec: DK-2). */
const SAID = 'the Passphrase is required';

test('lock the Library from a page of a book, and be told what it takes to open it', async ({
  page,
}) => {
  await page.goto('/');
  // A Library that is open: it has a name along the bottom, and a tree to walk.
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  const mapped = top(setting.album);
  await expect(row(page, mapped)).toBeVisible();

  // Down the tree to the book, the way the first journey walks it: a folder two
  // components deep is only reachable with the one above it open.
  await inTree(page, mapped).click();
  await inTree(page, leaf(setting.book)).click();
  await expect(row(page, bookPage(0))).toBeVisible();
  await shot(page, '01-the-book-as-a-folder');

  // And into it: a page opened and then turned, so the lock is pressed mid-book
  // rather than on the first thing the folder offered.
  await row(page, bookPage(0)).click();
  await expect(page.getByRole('img', { name: bookPage(0) })).toBeVisible();
  await page.keyboard.press('ArrowRight');
  await expect(page.getByRole('img', { name: bookPage(1) })).toBeVisible();
  await shot(page, '02-a-page-open-in-the-reader');

  // The reader covers the tree and the list and stops at the status bar, so
  // locking is something a person does without first closing the book.
  await page.getByRole('button', { name: 'lock', exact: true }).click();

  // No page is on the screen. The URL still names the one that was open — this
  // is a lock and not a navigation — and there is nothing behind it to draw: the
  // plaintext went with the key, and what the fresh request for it earns is the
  // refusal rather than the picture.
  //
  // The URL is asserted and not only said, because the empty screen does not
  // say it: a lock that had sent the screen back to the list instead would
  // leave exactly the same count of pictures behind it, and the count alone
  // would go on passing over an explorer that had stopped holding the place.
  await expect(page.getByRole('img')).toHaveCount(0);
  await expect(page).toHaveURL(
    new RegExp(`#path=${setting.book}&open=${setting.book}/${bookPage(1)}$`),
  );

  // And the reader is off the screen rather than standing over the list with
  // the asking inside it, which is what it is for the width of the round trip.
  // Its caption — which page, where in the book, and the keys — is drawn
  // whatever the page is doing, so the caption's absence is the reader's.
  await expect(page.getByText(`${bookPage(1)} (2/${setting.pages})`)).toHaveCount(0);

  // The screen asks its three questions again, and two of them are now refused.
  // The sentence is the server's own and is shown where every refusal on this
  // screen is shown — in the region that cannot answer, with the offer to ask
  // again beside it.
  await expect(page.getByText(SAID, { exact: false }).first()).toBeVisible();
  await expect(row(page, bookPage(0))).toHaveCount(0);
  await expect(page.locator('tbody tr')).toHaveCount(0);

  // It is not a screen to be stuck on: that offer is what a server started again
  // is met with, without the tab being reloaded — and the control that was
  // pressed is back at rest rather than left saying "locking…" over a Library
  // that is already shut.
  await expect(page.getByRole('button', { name: 'try again' }).first()).toBeVisible();
  await expect(page.getByRole('button', { name: 'lock', exact: true })).toBeEnabled();

  // And the Library still has a name. Which Library this is was never something
  // the Master Key kept, so a locked server goes on saying it rather than going
  // silent — otherwise a person with two of them open would have no way to tell
  // which one they had just shut.
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  await shot(page, '03-locked-and-the-reader-gone');
});
