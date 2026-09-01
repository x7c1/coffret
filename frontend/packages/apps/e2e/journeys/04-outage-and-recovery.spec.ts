// The server going away under an open explorer, and coming back.
//
// A page in a browser outlives the process it is reading from: the terminal it
// was started in gets closed, the machine sleeps, something falls over. What
// the screen owes then is a sentence and a way out, which is the one thing the
// spike this explorer replaced did not have — it logged its failure to the
// console and went on saying "loading" for as long as the tab was open.
//
// One of the two journeys that take the server away, and the one that asks what
// the screen does while it is gone; the restart journey asks what a server finds
// when it comes back. Each hands the server back before it ends, so the
// journeys after it run against one that is up.

import type { Locator, Page } from '@playwright/test';

import { bookPage, expect, row, setting, shot, test } from './journey';

/**
 * What the screen says when the server is not there.
 *
 * Two sentences and not one, because a server that has gone can go two ways
 * through the proxy the explorer is served behind: the connection is refused,
 * which never reaches an answer at all, or the proxy answers for it with a page
 * of its own that is not the server's refusal shape. Both are the same event
 * and both are what the explorer is being asked to survive here, so the journey
 * accepts either rather than pinning the run to which one a proxy happens to
 * do.
 */
function gone(page: Page): Locator {
  return page.getByText(
    /the coffret server did not answer|the server answered \d+ in a shape this client does not know/,
  );
}

test('lose the server and get the listing back', async ({ page, coffret }) => {
  await page.goto(`/#path=${setting.book}`);
  await expect(row(page, bookPage(0))).toBeVisible();

  await coffret.stop();
  await page.reload();

  // Every region of the screen asks the server the same question and they fail
  // together, so there is one offer to try again and it is made wherever the
  // eye lands.
  const again = page.getByRole('button', { name: 'try again' });
  await expect(again.first()).toBeVisible();
  await expect(gone(page).first()).toBeVisible();
  await shot(page, '01-the-server-is-gone');

  // Back on the same port, which is what the explorer is aimed at: a server
  // that came back somewhere else would be a server this page could not reach,
  // and the retry would be answering a different question.
  await coffret.start();
  await again.first().click();

  await expect(row(page, bookPage(0))).toBeVisible();
  await expect(page.getByText(`${setting.library} — on s3`)).toBeVisible();
  await expect(again).toHaveCount(0);
  await shot(page, '02-and-back-again');
});
