// The search field's More filters panel, opened while the reader is open.
//
// The panel hangs from the header over the right of the screen, where the
// reader's sheet is drawn. It has to be drawn in front of the sheet — not
// merely present in the page under it — so that it can be seen and used while
// a page is read. What it says about Recent waits behind its question mark ("About Recent").

import { expect, photo, row, setting, shot, test } from './journey';

test('open More filters over the reader, and turn Recent on from it', async ({ page }) => {
  await page.goto(`/#path=${setting.album}`);
  await row(page, photo(0)).dblclick();
  const reader = page.getByRole('region', { name: 'Reader' });
  await expect(reader.getByRole('img', { name: photo(0) })).toBeVisible();

  await page.getByRole('search').getByRole('button', { name: 'More filters' }).click();
  const panel = page.getByRole('group', { name: 'More filters' });
  await expect(panel).toBeVisible();

  // In front of the reader: what is drawn at the panel's centre, and at its
  // checkbox, is the panel's own.
  const box = await panel.boundingBox();
  const sheet = await reader.boundingBox();
  expect(box).not.toBeNull();
  expect(sheet).not.toBeNull();
  if (box === null || sheet === null) {
    return;
  }
  // The two overlap, or the check below would prove nothing.
  expect(box.x + box.width).toBeGreaterThan(sheet.x);
  expect(box.y + box.height).toBeGreaterThan(sheet.y);
  const onTop = await panel.evaluate((own, at) => {
    const hit = document.elementFromPoint(at.x, at.y);
    return hit !== null && own.contains(hit);
  }, { x: box.x + box.width / 2, y: box.y + box.height / 2 });
  expect(onTop).toBe(true);

  // What Recent does is out of sight until its question mark ("About Recent") is asked.
  const explainer = page.getByRole('tooltip');
  await expect(explainer).toHaveCount(0);
  await panel.getByRole('button', { name: 'About Recent' }).hover();
  await expect(explainer).toHaveText(
    'Folders that directly hold files, newest first by when their files were added.',
  );
  await shot(page, '01-more-filters-over-the-reader');

  // And it is used there: the press reaches the checkbox, and Recent is on.
  const recent = panel.getByRole('checkbox');
  await recent.click();
  await expect(recent).toBeChecked();
  await expect(page.getByTestId('recent-chip')).toHaveText(/Recent \d+/);
  await shot(page, '02-recent-on-over-the-reader');

  // Off again, from the chip, so the Library is left as the journey found it.
  await page.getByRole('button', { name: 'Turn off Recent' }).click();
  await expect(page.getByTestId('recent-chip')).toHaveCount(0);
});
