// Drawing the explorer in the dark set, and back.
//
// The switch at the header's right end, beside the +, opens a menu of three —
// System, Light and Dark — and the page is drawn in the one chosen at once:
// the ground, the header and the rows alike. The choice is this device's, kept
// by the server rather than in the tab, so a reload draws the page in it again.
// On System the page follows the system's own choice, as it switches. The
// journey ends on System, which is where it began.

import { expect, setting, shot, test, tree } from './journey';

test('switch the explorer to Dark and back to System', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto(`/#path=${setting.album}`);
  await expect(tree(page)).toBeVisible();
  const root = page.locator('html');
  const ground = () => page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  const button = page.getByRole('banner').getByRole('button', { name: /^Appearance/ });

  await expect(button).toHaveAccessibleName('Appearance: System');
  await expect(root).toHaveAttribute('data-scheme', 'light');
  const light = await ground();

  await button.click();
  const menu = page.getByRole('menu', { name: 'appearance' });
  await expect(menu.getByRole('menuitemradio', { name: 'System' })).toHaveAttribute('aria-checked', 'true');
  await menu.getByRole('menuitemradio', { name: 'Dark' }).click();
  await expect(menu).toHaveCount(0);
  await expect(button).toHaveAccessibleName('Appearance: Dark');
  await expect(root).toHaveAttribute('data-scheme', 'dark');
  expect(await ground()).not.toBe(light);
  await shot(page, '01-dark');

  // Kept for the device, not the tab's page: a reload is drawn dark again.
  await page.reload();
  await expect(tree(page)).toBeVisible();
  await expect(button).toHaveAccessibleName('Appearance: Dark');
  await expect(root).toHaveAttribute('data-scheme', 'dark');

  await button.click();
  await menu.getByRole('menuitemradio', { name: 'System' }).click();
  await expect(button).toHaveAccessibleName('Appearance: System');
  await expect(root).toHaveAttribute('data-scheme', 'light');
  expect(await ground()).toBe(light);

  // On System, the system's own switch is followed as it happens.
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(root).toHaveAttribute('data-scheme', 'dark');
  await shot(page, '02-system-dark');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(root).toHaveAttribute('data-scheme', 'light');

  await page.reload();
  await expect(tree(page)).toBeVisible();
  await expect(button).toHaveAccessibleName('Appearance: System');
});
