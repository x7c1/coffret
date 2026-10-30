// Bringing a scanned book in: the book's folder dropped onto a folder of the
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

import { cp, mkdtemp, readdir, readFile, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import type { Locator, Page } from '@playwright/test';

import {
  expect,
  glimpse,
  inTree,
  ON_THIS_DEVICE,
  openActivity,
  photo,
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

/** What the question offers for a Pack, chosen unless somebody says otherwise. */
const AS_ONE_PACK = /^One Pack/;

/**
 * Answers the confirmation a drop of the book `name` onto the album brings up:
 * it says where it lands — the album — names the folder with its count,
 * marked as a folder the Library does not have there yet, and goes as One
 * Pack on Add.
 */
async function addAsAPack(page: Page, name: string): Promise<void> {
  const question = page.getByRole('dialog', { name: `Add to ${setting.album}` });
  const incoming = question.getByRole('list', { name: 'incoming' });
  await expect(incoming).toContainText(`📁 ${name} — ${setting.importPages} files`);
  await expect(incoming).toContainText('New folder');
  await expect(question.getByRole('radio', { name: AS_ONE_PACK })).toBeChecked();
  await question.getByRole('button', { name: 'Add', exact: true }).click();
}

/**
 * The book's pages in a folder of their own called `name`, which is what the
 * drop carries: a dropped folder lands under its own name, so each journey
 * drops a copy named for it rather than the one folder the script made.
 */
async function bookCalled(name: string): Promise<string> {
  const folder = path.join(await mkdtemp(path.join(tmpdir(), 'coffret-book-')), name);
  await cp(setting.importDir, folder, { recursive: true });
  return folder;
}

/** One file under a dropped folder, as the page is handed it. */
interface Carried {
  /** Its path below the dropped folder, `/`-separated. */
  path: string;
  /** Its bytes, in base64 so they cross into the page as a string. */
  base64: string;
  /** Its modification time, in milliseconds from the Unix epoch. */
  modified: number;
}

/** Every file under `folder`, at every depth. */
async function carriedUnder(folder: string, under = ''): Promise<Carried[]> {
  const carried: Carried[] = [];
  for (const entry of await readdir(path.join(folder, under), { withFileTypes: true })) {
    const at = under === '' ? entry.name : `${under}/${entry.name}`;
    if (entry.isDirectory()) {
      carried.push(...(await carriedUnder(folder, at)));
    } else if (entry.isFile()) {
      const full = path.join(folder, at);
      carried.push({
        path: at,
        base64: (await readFile(full)).toString('base64'),
        modified: Math.trunc((await stat(full)).mtimeMs),
      });
    }
  }
  return carried;
}

/**
 * Drops `folder` onto `target` the way a browser hands a dropped folder to a
 * page: one entry for the folder, which walks to every file under it.
 *
 * Not through `dropFilesOnto`. Its protocol drag carries a folder's path, but
 * Chromium hands the page an entry for it whose reader fails with an
 * `EncodingError` — a drag made by the protocol registers no file system for
 * the folder the way one from the desktop does — so the explorer would be
 * refused the walk before it could ask anything. So the browser's half is
 * played here instead: the drag events are dispatched in the page, carrying an
 * entry answered from the folder's files as they are on this disk. Everything
 * from there on — walking the entries, counting them, the question, the
 * request — is the explorer's own.
 */
async function dropFolderOnto(target: Locator, folder: string): Promise<void> {
  const carried = await carriedUnder(folder);
  await target.evaluate(
    (element, { name, carried }) => {
      interface Held {
        name: string;
        folders: Map<string, Held>;
        files: File[];
      }
      const top: Held = { name, folders: new Map(), files: [] };
      for (const { path: at, base64, modified } of carried) {
        const parts = at.split('/');
        const base = parts.pop() ?? at;
        let here = top;
        for (const part of parts) {
          let next = here.folders.get(part);
          if (next === undefined) {
            next = { name: part, folders: new Map(), files: [] };
            here.folders.set(part, next);
          }
          here = next;
        }
        const bytes = Uint8Array.from(atob(base64), (char) => char.charCodeAt(0));
        here.files.push(new File([bytes], base, { lastModified: modified }));
      }
      const fileEntry = (file: File) => ({
        isFile: true,
        isDirectory: false,
        name: file.name,
        file: (resolve: (file: File) => void) => resolve(file),
      });
      const folderEntry = (held: Held): unknown => ({
        isFile: false,
        isDirectory: true,
        name: held.name,
        // One batch and then an empty one, as a reader ends.
        createReader: () => {
          let read = false;
          return {
            readEntries: (resolve: (entries: unknown[]) => void) => {
              const batch = read
                ? []
                : [...[...held.folders.values()].map(folderEntry), ...held.files.map(fileEntry)];
              read = true;
              resolve(batch);
            },
          };
        },
      });
      const transfer = {
        types: ['Files'],
        items: [{ kind: 'file', type: '', webkitGetAsEntry: () => folderEntry(top) }],
        files: [],
        dropEffect: 'copy',
        effectAllowed: 'copy',
      };
      for (const type of ['dragenter', 'dragover', 'drop']) {
        const event = new DragEvent(type, { bubbles: true, cancelable: true });
        Object.defineProperty(event, 'dataTransfer', { value: transfer });
        element.dispatchEvent(event);
      }
    },
    { name: path.basename(folder), carried },
  );
}

/** Drops `folder` onto the album's rows and answers the question with a Pack. */
async function dropAsAPack(page: Page, folder: string): Promise<void> {
  await page.goto(`/#path=${setting.album}`);
  await dropFolderOnto(row(page, photo(0)), folder);
  await addAsAPack(page, path.basename(folder));
}

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
  expect(await colourOf(page, STRANDED)).toBe(await colourOf(page, IMPORTED));
  await expect(stateIcon(page, pages[0])).toHaveAccessibleName(ON_THIS_DEVICE);
  await shot(page, '07-packed-at-the-second-attempt');
});
