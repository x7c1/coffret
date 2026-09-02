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
// the moment the pages land, and they become ordinary `present` rows when the
// batch commits. That the Containers behind them are Packs is not something a
// screen shows — the listing carries the kind and the explorer draws a state —
// so it is stated where it can be: the other device fetches the folder, and
// reads it out of fewer Containers than it has pages, which nothing carried in
// one Container per file could do (spec: PK-16).

import { readdir } from 'node:fs/promises';
import path from 'node:path';

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
import { fetchedElsewhere } from './uploader';

/** What the folder made in the browser is called. */
const IMPORTED = 'imported-in-the-browser';

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
  // Which word each chip has for it is the machine's speed — `uploading` until
  // the batch commits, `present` where the freeze outran the first frame — so
  // the rows are asserted and the packing moment is photographed where it is
  // caught.
  await expect(page.locator('tbody tr')).toHaveCount(setting.importPages);
  await expect(chip(page, pages[0])).toHaveText(/^(uploading|present)$/);
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

  // And the other device reads it back, which is what says the book is in the
  // Library rather than on this disk. It fetches every page — and out of fewer
  // Containers than there are pages, because the fetch unit is the whole
  // Container however many of its Entries were wanted (spec: PK-16). A folder
  // carried in one Container per page would answer with one each.
  const elsewhere = await fetchedElsewhere(setting, `${mapped}/${IMPORTED}`);
  expect(elsewhere.entries).toBe(setting.importPages);
  expect(elsewhere.containers).toBeLessThan(setting.importPages);
});
