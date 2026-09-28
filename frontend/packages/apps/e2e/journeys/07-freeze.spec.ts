// Bringing a scanned book in: a folder made in the browser, and its pages
// dropped into it.
//
// This is the daily gesture the explorer exists for. A book is one folder of
// page images, and adding it the way a photograph is added would make it one
// Container per page — a few hundred uploads, a few hundred objects, and a few
// hundred provider calls to open it again. So a drop into a folder somebody just
// made is read as a book: the pages land, a freeze packs them together, and what
// the Library gains is Packs (spec: PK-1, PK-7, PK-17).
//
// What a person sees is the whole of what this journey asserts: the folder they
// made is on the tree though the Library has never heard of it, the rows appear
// the moment the pages land, they become ordinary `present` rows when the batch
// commits, and a reload finds the folder as the Library's own rather than this
// browser's. That the Containers behind them are Packs is not something a
// screen shows — the listing carries the kind and the explorer draws a state —
// so it is not asserted here. The API stage of `scripts/e2e-it.sh` owns those
// Storage-side facts: it checks every page of a dropped book is in a Pack, and
// that the other device reads the book back out of fewer Containers than it has
// pages (spec: PK-16).
//
// And the other end a book can come to: Storage stopping its freeze, which the
// second journey here reloads over.

import { readdir } from 'node:fs/promises';
import path from 'node:path';

import type { Locator } from '@playwright/test';

import {
  chip,
  dropFilesOnto,
  expect,
  glimpse,
  inTree,
  setting,
  shot,
  test,
  top,
} from './journey';
import { startStorage, stopStorage } from './storage';

/** What the folder made in the browser is called. */
const IMPORTED = 'imported-in-the-browser';

/** What the folder whose freeze Storage stops is called. */
const STRANDED = 'stopped-in-the-browser';

/** How long the staged state is waited for before the first picture. */
const GLIMPSE_MS = 4_000;

/** How long the book has to reach MinIO and be committed. */
const FREEZE_MS = 120_000;

test('make a folder, drop a book into it, and watch it pack', async ({ page }) => {
  const mapped = top(setting.album);
  const pages = (await readdir(setting.importDir)).sort();
  expect(pages).toHaveLength(setting.importPages);

  // Into the part of the Library this device maps, because that is the only part
  // a file can land in at all (spec: EP-9).
  await page.goto(`/#path=${mapped}`);
  page.once('dialog', (asking) => void asking.accept(IMPORTED));
  await page.getByRole('button', { name: `new folder in ${mapped}` }).click();

  // The screen is in it, and the tree draws it — though the Library has never
  // heard of it. A Library has no folders to make: a folder is the separators in
  // the Entry Paths under it, so until the first page commits this place is the
  // browser's alone.
  await expect(page).toHaveURL(new RegExp(`#path=${mapped}/${IMPORTED}$`));
  await expect(inTree(page, IMPORTED)).toBeVisible();
  await expect(page.getByText(/this folder was made here/)).toBeVisible();
  await shot(page, '01-a-folder-made-here');

  // The book, dropped whole. One gesture, one request, and the pages named
  // relative to the folder they land in.
  await dropFilesOnto(
    page,
    page.getByText(/this folder was made here/),
    pages.map((name) => path.join(setting.importDir, name)),
  );

  // In the folder from that moment, whatever the freeze has managed by then.
  // Which word each chip has for it is the machine's speed — "not in Library"
  // until the batch commits, `present` where the freeze outran the first
  // frame — so the rows are asserted and the packing moment is photographed
  // where it is caught.
  await expect(page.locator('tbody tr')).toHaveCount(setting.importPages);
  await expect(chip(page, pages[0])).toHaveText(/^(not in Library|present)$/);
  await glimpse(page.getByText(/packing this folder/), GLIMPSE_MS);
  await shot(page, '02-the-pages-landed');

  // And when the batch commits they are ordinary rows: the Library holds the
  // Entries, and this device has the files they were made from.
  for (const name of pages) {
    await expect(chip(page, name)).toHaveText('present', { timeout: FREEZE_MS });
  }
  await expect(page.getByText(new RegExp(`packed ${setting.importPages} files`))).toBeVisible();
  await shot(page, '03-packed-into-the-library');

  // The folder is the Library's now rather than this browser's: the first Entry
  // under it committed, so the server names it, and a reload finds it there.
  await page.reload();
  await expect(page.getByText(/this folder was made here/)).toHaveCount(0);
  await expect(inTree(page, IMPORTED)).toBeVisible();
  await expect(page.locator('tbody tr')).toHaveCount(setting.importPages);
  await shot(page, '04-an-ordinary-folder-of-the-library');
});

/** The colour a folder's name is drawn in on the tree. */
function colourOf(name: Locator): Promise<string> {
  return name.evaluate((element) => getComputedStyle(element).color);
}

// A book whose freeze Storage stopped is a folder full of pages on the disk and
// out of the Library, and the folder was never anything but this browser's. So
// a reload is where it would go — no row to walk into, nothing to pack again —
// unless the explorer takes it back out of what the server is still holding
// about the freeze. This is that reload, and the second attempt it leaves
// reachable.
test('lose Storage mid-book, reload, and pack the book again', async ({ page }) => {
  const mapped = top(setting.album);
  const pages = (await readdir(setting.importDir)).sort();

  await page.goto(`/#path=${mapped}`);
  page.once('dialog', (asking) => void asking.accept(STRANDED));
  await page.getByRole('button', { name: `new folder in ${mapped}` }).click();
  await expect(page).toHaveURL(new RegExp(`#path=${mapped}/${STRANDED}$`));

  // Storage goes away before the book arrives, so the pages land on the disk
  // and the freeze that would carry them in is refused by the first thing it
  // asks Storage for.
  await stopStorage(setting);
  try {
    await dropFilesOnto(
      page,
      page.getByText(/this folder was made here/),
      pages.map((name) => path.join(setting.importDir, name)),
    );
    await expect(page.locator('tbody tr')).toHaveCount(setting.importPages);
    await expect(page.getByText(/could not pack/)).toBeVisible({ timeout: FREEZE_MS });
    await shot(page, '05-storage-stopped-the-book');

    // A reload. Nothing in the URL or in this tab remembers the folder; the
    // server's answer about the freeze it stopped is what brings it back —
    // on the tree, drawn dimmed beside the folder the Library does hold,
    // with its pages in it and the second attempt on offer.
    await page.reload();
    await expect(inTree(page, STRANDED)).toBeVisible();
    await expect(inTree(page, IMPORTED)).toBeVisible();
    expect(await colourOf(inTree(page, STRANDED))).not.toBe(
      await colourOf(inTree(page, IMPORTED)),
    );
    await expect(page.locator('tbody tr')).toHaveCount(setting.importPages);
    await expect(chip(page, pages[0])).toHaveText('not in Library');
    await expect(page.getByRole('button', { name: 'pack again' })).toBeVisible();
    await shot(page, '06-the-folder-came-back');
  } finally {
    // Whatever became of the steps above, the journeys after this one are
    // walked against a Storage that is up.
    await startStorage(setting);
  }

  // Storage is back, and the second attempt packs what was sitting there.
  await page.getByRole('button', { name: 'pack again' }).click();
  for (const name of pages) {
    await expect(chip(page, name)).toHaveText('present', { timeout: FREEZE_MS });
  }
  await expect(page.getByText(new RegExp(`packed ${setting.importPages} files`))).toBeVisible();

  // And it is the Library's folder now, drawn like the one beside it.
  await page.reload();
  await expect(page.getByText(/this folder was made here/)).toHaveCount(0);
  await expect(inTree(page, STRANDED)).toBeVisible();
  expect(await colourOf(inTree(page, STRANDED))).toBe(await colourOf(inTree(page, IMPORTED)));
  await shot(page, '07-packed-at-the-second-attempt');
});
