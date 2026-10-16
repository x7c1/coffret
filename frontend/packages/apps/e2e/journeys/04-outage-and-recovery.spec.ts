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
//
// The explorer's host is what notices. A server it finds stopped is the end of
// every session it holds (spec: LA-15), so the screen goes with the session:
// the tab says it holds no session, and how the explorer is opened. What brings
// the listing back is what brings it back for a person — the server started
// again, and the launch file the host writes for its next run opened.

import {
  bookPage,
  expect,
  libraryName,
  NO_SESSION,
  openExplorer,
  row,
  setting,
  shot,
  test,
} from './journey';

test('lose the server and get the listing back', async ({ page, coffret }) => {
  await page.goto(`/#path=${setting.book}`);
  await expect(row(page, bookPage(0))).toBeVisible();

  await coffret.stop();
  await page.reload();

  // The host found the server gone as the page asked it, and ended every
  // session it held. Which sentence comes with that — the server not
  // answering, or the session simply gone — is whichever of the page's
  // questions was answered first, so the journey asserts the state rather than
  // the words.
  await expect(page.getByText(NO_SESSION)).toBeVisible();
  await expect(row(page, bookPage(0))).toHaveCount(0);
  await shot(page, '01-the-server-is-gone');

  // Back on the same port, which is what the host is aimed at: a server that
  // came back somewhere else would be a server this page could not reach. The
  // host sees its new key and writes a launch file for it.
  await coffret.start();
  await openExplorer(page, { afterRestart: true });
  await page.goto(`/#path=${setting.book}`);

  await expect(row(page, bookPage(0))).toBeVisible();
  await expect(libraryName(page)).toBeVisible();
  await shot(page, '02-and-back-again');
});
