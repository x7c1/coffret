// A book's pages dropped onto the album as a folder, and added as a Pack —
// the gesture the freeze journey and the stop journey both make.

import { cp, mkdtemp, readdir, readFile, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import type { Locator, Page } from '@playwright/test';

import { expect, photo, row, setting } from './journey';

/** What the question offers for a Pack, chosen unless somebody says otherwise. */
const AS_ONE_PACK = /^One Pack/;

/**
 * Answers the confirmation a drop of the book `name` onto the album brings up:
 * it says where it lands — the album — names the folder with its count,
 * marked as a folder the Library does not have there yet, and goes as One
 * Pack on Add.
 */
export async function addAsAPack(page: Page, name: string): Promise<void> {
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
export async function bookCalled(name: string): Promise<string> {
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
export async function dropFolderOnto(target: Locator, folder: string): Promise<void> {
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
export async function dropAsAPack(page: Page, folder: string): Promise<void> {
  await page.goto(`/#path=${setting.album}`);
  await dropFolderOnto(row(page, photo(0)), folder);
  await addAsAPack(page, path.basename(folder));
}

