// Bringing in a book whose pages are image files: the book's folder dropped onto a folder of the
// Library, and added as a Pack.
//
// This is the daily gesture the explorer exists for. A book is one folder of
// page images, and adding it the way a photograph is added would make it one
// Container per page — a few hundred uploads, a few hundred objects, and a few
// hundred provider calls to open it again. So a drop holding a folder is asked
// about before anything is sent, as every drop is confirmed: naming the folder
// it lands in and the dropped folder with its count and size, and offering it
// as One Pack. Adding it as a Pack packs exactly the pages the drop carried: they
// land, a freeze packs them together, and what the Library gains is Packs
// (spec: PK-1, PK-7, PK-17).
//
// What a person sees is the whole of what this journey asserts: the question
// names where it lands, the dropped folder and how many files it holds — a
// folder the Library does not have there yet, marked New folder — the folder is on the
// tree though the Library has not committed it yet, the rows appear the moment
// the pages land, they become ordinary rows on this device when the batch commits,
// and a reload finds the folder as the Library's own rather than this
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

import type { Page } from '@playwright/test';

import { addAsAPack, bookCalled, dropAsAPack, dropFolderOnto } from './books';
import {
  expect,
  glimpse,
  inTree,
  ON_THIS_DEVICE,
  openActivity,
  row,
  rowsNamed,
  setting,
  shot,
  stateIcon,
  test,
  TO_BE_PACKED,
} from './journey';
import { startStorage, stopStorage } from './storage';

/** What the book dropped in the first journey is called. */
const IMPORTED = 'imported-in-the-browser';

/** What the book whose freeze Storage stops is called. */
const STRANDED = 'stopped-in-the-browser';

/** How long the staged state is waited for before the first picture. */
const GLIMPSE_MS = 4_000;

/** How long the book has to reach MinIO and be committed. */
const FREEZE_MS = 120_000;

test('drop a book folder, add it as a Pack, and watch it pack', async ({ page }) => {
  const pages = (await readdir(setting.importDir)).sort();
  expect(pages).toHaveLength(setting.importPages);

  // Onto the album, which is in the part of the Library this device maps,
  // because that is the only part a file can land in at all (spec: EP-9) — and
  // onto its row in the tree, which takes a drop for its own folder as the
  // list does. The question is the first thing the drop is answered with:
  // nothing is sent until it is.
  const book = await bookCalled(IMPORTED);
  await page.goto(`/#path=${setting.album}`);
  await dropFolderOnto(inTree(page, path.basename(setting.album)), book);
  await expect(page.getByRole('dialog', { name: `Add to ${setting.album}` })).toContainText(
    `📁 ${IMPORTED} —`,
  );
  await shot(page, '01-asked-how-to-add-the-folder');
  await addAsAPack(page, IMPORTED);

  // The folder is on the tree though the Library has not committed it: the
  // freeze packing it names it, and the explorer draws it from that.
  await expect(inTree(page, IMPORTED)).toBeVisible({ timeout: FREEZE_MS });
  await page.goto(`/#path=${setting.album}/${IMPORTED}`);

  // In the folder from that moment, whatever the freeze has managed by then.
  // Which state each icon says is the machine's speed — being added until the
  // batch commits, on this device where the freeze outran the first frame — so
  // the rows are asserted and the packing moment is photographed where it is
  // caught.
  await expect(rowsNamed(page, pages)).toHaveCount(setting.importPages);
  await expect(stateIcon(page, pages[0])).toHaveAccessibleName(/^On this device/);
  await glimpse(page.getByText(/packing this folder/), GLIMPSE_MS);
  await shot(page, '02-the-pages-landed');

  // And when the batch commits they are ordinary rows: the Library holds the
  // Entries, and this device has the files they were made from.
  for (const name of pages) {
    await expect(stateIcon(page, name)).toHaveAccessibleName(ON_THIS_DEVICE, { timeout: FREEZE_MS });
  }
  await expect(page.getByText(new RegExp(`packed ${setting.importPages} files`))).toBeVisible();
  await shot(page, '03-packed-into-the-library');

  // The folder is the Library's now: the first Entry under it committed, so the
  // server names it, and a reload finds it there.
  await page.reload();
  await expect(inTree(page, IMPORTED)).toBeVisible();
  await expect(rowsNamed(page, pages)).toHaveCount(setting.importPages);
  await shot(page, '04-an-ordinary-folder-of-the-library');
});

/**
 * Puts the focus on the page row `name`, off both folders: the focused row is
 * drawn in white on the accent whatever it is, so two folders' names are only
 * told apart by colour where neither holds the focus — and the reload lands it
 * on the folder in the address.
 */
async function focusAPage(page: Page, name: string): Promise<void> {
  await row(page, name).click();
  await expect(row(page, name)).toHaveAttribute('data-focused', '');
}

/** The colour a folder's name is drawn in on the tree. */
function colourOf(page: Page, name: string): Promise<string> {
  return inTree(page, name)
    .getByText(name, { exact: true })
    .evaluate((element) => getComputedStyle(element).color);
}

// A book whose freeze Storage stopped is a folder full of pages on the disk and
// out of the Library, and the Library has never named the folder. So a reload is
// where it would go — no row to walk into, nothing to pack again — unless the
// explorer takes it back out of what the server is still holding about the
// freeze. This is that reload, and the second attempt it leaves reachable.
test('lose Storage mid-book, reload, and pack the book again', async ({ page }) => {
  const pages = (await readdir(setting.importDir)).sort();
  const book = await bookCalled(STRANDED);

  // Storage goes away before the book arrives, so the pages land on the disk
  // and the freeze that would carry them in is refused by the first thing it
  // asks Storage for.
  await stopStorage(setting);
  try {
    await dropAsAPack(page, book);
    await expect(inTree(page, STRANDED)).toBeVisible({ timeout: FREEZE_MS });
    await page.goto(`/#path=${setting.album}/${STRANDED}`);
    await expect(rowsNamed(page, pages)).toHaveCount(setting.importPages);
    await expect(page.getByText(/could not pack/)).toBeVisible({ timeout: FREEZE_MS });
    await shot(page, '05-storage-stopped-the-book');

    // A reload. Nothing in this tab remembers the folder; the server's answer
    // about the freeze it stopped is what brings it back — on the tree, drawn
    // dimmed beside the folder the Library does hold, with its pages in it and
    // the second attempt on offer. A run that stops keeps the drop's
    // reservation (spec: PK-17), so the pages are still the book's, waiting
    // for their Pack, and a sync would leave them alone.
    await page.reload();
    await expect(inTree(page, STRANDED)).toBeVisible();
    await expect(inTree(page, IMPORTED)).toBeVisible();
    await focusAPage(page, pages[0]);
    expect(await colourOf(page, STRANDED)).not.toBe(await colourOf(page, IMPORTED));
    await expect(rowsNamed(page, pages)).toHaveCount(setting.importPages);
    await expect(stateIcon(page, pages[0])).toHaveAccessibleName(TO_BE_PACKED);
    // The second attempt is the Retry on the stopped freeze's entry in the
    // Activity panel, which the server's log kept across the reload.
    await openActivity(page);
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
    await expect(stateIcon(page, name)).toHaveAccessibleName(ON_THIS_DEVICE, { timeout: FREEZE_MS });
  }
  // Named by its folder: the panel is open, and it lists the first journey's
  // book packed in the same words beside this one.
  await expect(
    page.getByText(new RegExp(`packed ${setting.importPages} files of .*${STRANDED}`)),
  ).toBeVisible();

  // And it is the Library's folder now, drawn like the one beside it; the
  // commit released the reservation, so a reload finds its pages present
  // rather than still to be packed.
  await page.reload();
  await expect(inTree(page, STRANDED)).toBeVisible();
  await focusAPage(page, pages[0]);
  expect(await colourOf(page, STRANDED)).toBe(await colourOf(page, IMPORTED));
  await expect(stateIcon(page, pages[0])).toHaveAccessibleName(ON_THIS_DEVICE);
  await shot(page, '07-packed-at-the-second-attempt');
});
