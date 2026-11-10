// The filter kept across a reload.
//
// What the tree is filtered by — the name in the header's search field,
// whether Recent is on, and its count — is this device's, kept by the server
// beside the appearance rather than in the tab (spec: LA-43), so a reload of
// the page (or the desktop app started again) opens the explorer filtered
// exactly as it was left: the name in the field, the `Recent n ×` chip, the
// panel's count, and the tree drawn from the same answer. The journey ends
// unfiltered, and the server keeps that too, so the journeys after it open
// unfiltered.

import type { Page } from '@playwright/test';

import { clearFilter, expect, filterKept, leaf, setting, shot, test, tree } from './journey';

/** The rows the tree draws, top to bottom, each by its key. */
function drawn(page: Page): Promise<string[]> {
  return tree(page)
    .locator('tbody tr[data-key]')
    .evaluateAll((rows) => rows.map((one) => one.getAttribute('data-key') ?? ''));
}

/** What the filter's line says once it has been answered: a count, or none. */
const ANSWERED = /^(\d[\d,]* match(es)?|No matches)/;

test('a filter set before a reload is the filter after it, until it is cleared', async ({ page }) => {
  const album = leaf(setting.album);
  // The root, with nothing opened: the filtered tree is then the answer alone.
  // The page opens unfiltered whatever a journey before this left kept (see
  // the `page` fixture).
  await page.goto('/');
  await expect(tree(page)).toBeVisible();
  const field = page.getByRole('search').getByRole('textbox', { name: 'Filter by name' });
  const status = page.getByRole('status', { name: 'filter' });
  const chip = page.getByTestId('recent-chip');
  const more = page.getByRole('button', { name: 'More filters' });
  const panel = page.getByRole('group', { name: 'More filters' });
  const count = panel.getByRole('spinbutton', { name: 'How many recent folders' });
  await expect(status).toHaveCount(0);
  await expect(field).toHaveValue('');

  // A name, then Recent on with a count of 2, kept as each is set.
  await field.fill(album);
  await expect(status).toContainText(ANSWERED);
  await more.click();
  await count.fill('2');
  const recentKept = filterKept(page, (kept) => kept.name === album && kept.recentOn && kept.recent === 2);
  // The line read once the tree is drawn from the answer for both, not from
  // the one for the name alone still standing while Recent is asked.
  const recentAnswered = page.waitForResponse((answer) => {
    const asked = new URL(answer.url());
    return asked.pathname === '/api/filter' && asked.searchParams.get('recent') === '2';
  });
  await panel.getByRole('checkbox').check();
  await recentKept;
  await recentAnswered;
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(chip).toHaveText(/Recent 2/);
  await expect(tree(page).getByRole('columnheader', { name: 'Added' })).toBeVisible();
  await expect(status).toContainText(ANSWERED);
  const line = await status.textContent();
  const rows = await drawn(page);
  expect(rows.length).toBeGreaterThan(0);
  await shot(page, '01-before-the-reload');

  // Reloaded, the explorer opens filtered as it was left.
  await page.reload();
  await expect(tree(page)).toBeVisible();
  await expect(field).toHaveValue(album);
  await expect(chip).toHaveText(/Recent 2/);
  await expect(status).toHaveText(line ?? '');
  await expect.poll(() => drawn(page)).toEqual(rows);
  await more.click();
  await expect(panel.getByRole('checkbox')).toBeChecked();
  await expect(count).toHaveValue('2');
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await shot(page, '02-after-the-reload');

  // Cleared, and kept cleared, so the next page opens unfiltered.
  await clearFilter(page);
  await expect(chip).toHaveCount(0);
  await expect(field).toHaveValue('');
  await page.reload();
  await expect(tree(page)).toBeVisible();
  await expect(field).toHaveValue('');
  await expect(chip).toHaveCount(0);
  await expect(status).toHaveCount(0);
});
